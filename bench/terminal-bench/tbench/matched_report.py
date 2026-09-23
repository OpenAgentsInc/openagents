"""Recompute matched-run measurements from retained, digested evidence."""

from __future__ import annotations

import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path
import statistics

from .matched import EXPERIMENT, protocol
from .paths import PACKAGE_DIR
from .usage_limit import scan_stream
from .matched_regrade import digests


def read(path: Path) -> dict:
    return json.loads(path.read_text()) if path.exists() else {}


def seconds(record: dict) -> float | None:
    if not record.get("started_at") or not record.get("finished_at"):
        return None
    return (datetime.fromisoformat(record["finished_at"].replace("Z", "+00:00"))
            - datetime.fromisoformat(record["started_at"].replace("Z", "+00:00"))).total_seconds()


def native(path: Path) -> dict:
    events = []
    invalid = 0
    text = path.read_text()
    for line in text.splitlines():
        try:
            events.append(json.loads(line))
        except ValueError:
            invalid += 1
    init = next((e for e in events if e.get("type") == "system"
                 and e.get("subtype") == "init"), {})
    final = next((e for e in reversed(events) if e.get("type") == "result"), {})
    calls, messages = set(), set()
    for event in events:
        if event.get("type") != "assistant":
            continue
        message = event.get("message") or {}
        if message.get("id"):
            messages.add(message["id"])
        for block in message.get("content") or []:
            if isinstance(block, dict) and block.get("type") == "tool_use":
                calls.add(block["id"])
    return {"path": str(path.relative_to(PACKAGE_DIR.parent.parent)),
            "initialization": {k: init.get(k) for k in
                ("model", "tools", "permissionMode", "claude_code_version", "cwd")},
            "models_billed": list((final.get("modelUsage") or {}).keys()),
            "turns": final.get("num_turns"), "model_messages": len(messages),
            "tool_calls": len(calls), "cost_usd": final.get("total_cost_usd"),
            "duration_ms": final.get("duration_ms"),
            "api_duration_ms": final.get("duration_api_ms"),
            "usage": final.get("usage"), "is_error": final.get("is_error"),
            "terminal_reason": final.get("terminal_reason"),
            "invalid_json_lines": invalid, "usage_limit": scan_stream(text)}


def verify_retention(episode: Path) -> dict:
    retention = read(episode / "retention.json")
    for entry in retention.get("files", []):
        path = episode / entry["path"]
        if entry["kind"] == "normalized trajectory":
            path = episode.parent / entry["path"]
        assert path.is_file(), path
        assert hashlib.sha256(path.read_bytes()).hexdigest() == entry["sha256"], path
        if entry.get("manifest_sha256"):
            assert entry["sha256"] == entry["manifest_sha256"], path
    assert retention.get("credential_scan", {}).get("matches") == 0, episode
    return retention


def trial_row(episode: Path, arm: str, task: str, repetition: int) -> dict:
    p = protocol()
    result = read(episode / "harbor-result.json")
    retention = verify_retention(episode)
    streams = sorted(episode.glob("artifacts/*.stream.jsonl")) if arm == "coder" else [episode / "native/claude-code.txt"]
    sessions = [native(path) for path in streams if path.is_file()]
    usage = read(episode / "evaluation/usage.json")
    composition = read(episode / "artifacts/composition.json")
    expected_system = hashlib.sha256((EXPERIMENT / "system-prompt.md").read_bytes()).hexdigest()
    systems = sorted(episode.glob("artifacts/*.system.md")) if arm == "coder" else [episode / "native/system-prompt.txt"]
    system_digests = [hashlib.sha256(path.read_bytes()).hexdigest() for path in systems if path.exists()]
    if arm == "coder":
        document = read(episode / "trajectory.atif.json")
        instruction = next((s.get("message") for s in document.get("steps", [])
                            if s.get("source") == "user"), None)
    else:
        path = episode / "native/instruction.txt"
        instruction = path.read_text() if path.exists() else None
    violations = []
    if not sessions:
        violations.append("No native stream retained")
    if instruction is None:
        violations.append("No task instruction retained")
    if not system_digests or any(value != expected_system for value in system_digests):
        violations.append("Missing or different system prompt")
    for session in sessions:
        init = session["initialization"]
        if (init["model"] != p["model"] or init["claude_code_version"] != p["claude_version"]
                or sorted(init["tools"] or []) != sorted(p["tools"].split(","))
                or init["permissionMode"] != "bypassPermissions"):
            violations.append("Native initialization differs from the executor controls")
        if session["models_billed"] != [p["model"]]:
            violations.append("Billed models missing or different")
    if arm == "coder":
        for branch in composition.get("branches", []):
            tier = branch["tier"]
            if any(tier.get(k) != p[k] for k in ("model", "effort", "tools")):
                violations.append("Different executor controls")
            if tier.get("prompt_cache_ttl") != p["cache_ttl"]:
                violations.append("Different cache lifetime")
        cost = usage.get("cost", {}).get("amount_usd")
        unpriced = usage.get("cost", {}).get("unknown_calls")
    else:
        invocation = read(episode / "native/invocation.txt")
        argv = invocation.get("argv", [])
        if "--effort" not in argv or argv[argv.index("--effort") + 1] != p["effort"]:
            violations.append("Missing or different plain executor effort")
        if invocation.get("environment", {}).get("CLAUDE_CODE_PROMPT_CACHE_TTL") != p["cache_ttl"]:
            violations.append("Missing or different plain cache lifetime")
        cost = (result.get("agent_result") or {}).get("cost_usd")
        unpriced = int(cost is None)
    return {"job": episode.parent.name, "trial": result.get("trial_name"), "task": task,
            "arm": arm, "repetition": repetition,
            "started_at": result.get("started_at"), "finished_at": result.get("finished_at"),
            "evidence": str(episode.relative_to(PACKAGE_DIR.parent.parent)),
            "reward": (result.get("verifier_result") or {}).get("rewards", {}).get("reward"),
            "exception": result.get("exception_info"),
            "usage_limited": any(s["usage_limit"] for s in sessions),
            "agent_seconds": seconds(result.get("agent_execution") or {}),
            "trial_seconds": seconds(result), "setup_seconds": seconds(result.get("agent_setup") or {}),
            "cost_usd": cost, "unpriced_calls": unpriced,
            "controller_usage": usage.get("calls"),
            "controller_cost_usd": (usage.get("components") or {}).get("jev", {}).get("cost_usd"),
            "checks": composition.get("checks"), "repair": composition.get("repair"),
            "branches": composition.get("branches"), "horizon": composition.get("horizon"),
            "sessions": sessions, "system_sha256": system_digests,
            "instruction_sha256": hashlib.sha256(instruction.encode()).hexdigest() if instruction is not None else None,
            "matching_violations": violations, "retention_missing": retention.get("missing"),
            "verified_files": len(retention.get("files", []))}


def aggregate(selected: list[dict]) -> dict:
    summary = {"started_trials": len(selected),
               "original_infrastructure_exceptions": sum(bool(r["exception"]) for r in selected),
               "grades_recovered_without_inference": sum("recovered_grade" in r for r in selected),
               "passes": sum(not r["usage_limited"] and (
                   (r["reward"] == 1 and not r["exception"])
                   or r.get("recovered_grade", {}).get("reward") == 1) for r in selected)}
    for metric in ["cost_usd", "agent_seconds", "trial_seconds"]:
        values = [r[metric] for r in selected]
        complete = bool(values) and all(v is not None for v in values)
        summary[metric] = {"total": sum(values) if complete else None,
                           "mean": statistics.mean(values) if complete else None,
                           "range": [min(values), max(values)] if complete else None}
    return summary


def report(traces: Path) -> dict:
    p = protocol()
    rows, absent = [], []
    for repetition, order in enumerate(p["orders"], 1):
        for task in p["tasks"]:
            for arm in order:
                job = f"{p['id']}--{task}--r{repetition}--{arm}"
                episodes = sorted((traces / job).glob("*.episode"))
                if not episodes:
                    absent.append(job)
                for episode in episodes:
                    row = trial_row(episode, arm, task, repetition)
                    replays = list((traces / (job + "-regrade")).glob("*.episode"))
                    if replays:
                        assert len(replays) == 1 and row["reward"] is None
                        replay = replays[0]
                        replay_retention = verify_retention(replay)
                        replay_record = read(replay / "native/artifact-replay.txt")
                        assert replay_record["model_calls"] == 0
                        assert replay_record["candidate_edits"] == 0
                        expected = replay_record["verified_sha256"]
                        assert digests(episode / "produced/app/evalbench") == expected
                        assert digests(replay / "produced/app/evalbench") == expected
                        grade = read(replay / "harbor-result.json")
                        assert grade["exception_info"] is None
                        row["recovered_grade"] = {
                            "reward": grade["verifier_result"]["rewards"]["reward"],
                            "evidence": str(replay.relative_to(PACKAGE_DIR.parent.parent)),
                            "model_calls": 0, "unchanged_files_verified": len(expected),
                            "verified_files": len(replay_retention["files"]),
                            "regrade_trial_seconds": seconds(grade)}
                        row["original_trial_seconds"] = row["trial_seconds"]
                        row["trial_seconds"] = None  # No comparable uninterrupted full trial.
                    rows.append(row)
    aggregates = {}
    for task in ["all", *p["tasks"]]:
        aggregates[task] = {}
        for arm in ["plain", "coder"]:
            selected = [r for r in rows if r["arm"] == arm and (task == "all" or r["task"] == task)]
            aggregates[task][arm] = aggregate(selected)
    recovered_pairs = {(r["task"], r["repetition"]) for r in rows if "recovered_grade" in r}
    sensitivity = {arm: aggregate([r for r in rows if r["arm"] == arm and
                   (r["task"], r["repetition"]) not in recovered_pairs]) for arm in ("plain", "coder")}
    instructions = {task: sorted({r["instruction_sha256"] for r in rows if r["task"] == task
                                 and r["instruction_sha256"] is not None}) for task in p["tasks"]}
    for row in rows:
        if len(instructions[row["task"]]) != 1:
            row["matching_violations"].append("Task instructions differ between attempts")
    return {"schema": "openagents.tbench.matched-results.v1", "protocol": p,
            "complete": not absent and len(rows) == 12, "not_retained": absent,
            "trials": rows, "aggregates": aggregates,
            "task_instruction_sha256": instructions,
            "sensitivity_excluding_recovered_pairs": sensitivity}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--traces", type=Path, default=PACKAGE_DIR / "traces")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.write_text(json.dumps(report(args.traces), indent=2) + "\n")


if __name__ == "__main__":
    main()
