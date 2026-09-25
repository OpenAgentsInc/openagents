"""Grade retained sequential candidates after execution, without model calls.

Deduplication is explicit and local to one batch. It assumes a deterministic
verifier in an unchanged Docker environment; a reused grade is not a second
independent verifier execution. No grades are sent back into an agent.
"""
from __future__ import annotations

import hashlib
import json
import math
import re
import shutil
import stat
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Callable

from . import replay

SCHEMA = "openagents.tbench.candidates.v2"
EXCLUDED = {".git", "__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache"}
MAX_FILES = 20_000
MAX_BYTES = 256 * 1024 * 1024


def digest(value) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def inventory(root: Path) -> dict:
    """A bounded complete inventory; refuse symlinks and special entries."""
    if not root.is_dir() or root.is_symlink():
        raise replay.ReplayError(f"not a plain candidate/task directory: {root}")
    entries = {".": {"kind": "directory", "mode": stat.S_IMODE(root.stat().st_mode)}}
    size = 0
    stack = [root]
    while stack:
        directory = stack.pop()
        for path in sorted(directory.iterdir()):
            info = path.lstat()
            relative = path.relative_to(root).as_posix()
            if stat.S_ISDIR(info.st_mode):
                entries[relative] = {"kind": "directory", "mode": stat.S_IMODE(info.st_mode)}
                stack.append(path)
            elif stat.S_ISREG(info.st_mode):
                size += info.st_size
                if size > MAX_BYTES:
                    raise replay.ReplayError("candidate/task exceeds the 256 MiB inventory bound")
                with path.open("rb") as stream:
                    content = hashlib.file_digest(stream, "sha256").hexdigest()
                entries[relative] = {"kind": "file", "mode": stat.S_IMODE(info.st_mode), "sha256": content}
            else:
                raise replay.ReplayError(f"unsupported symlink or special entry: {path}")
            if len(entries) > MAX_FILES:
                raise replay.ReplayError("candidate/task exceeds the 20,000-entry inventory bound")
    return entries


def recorded_files(entries: dict) -> dict:
    return {name: entry["sha256"] for name, entry in entries.items()
            if entry["kind"] == "file" and not name.endswith(".pyc")
            and not any(part in EXCLUDED for part in Path(name).parts[:-1])}


def discover_with_errors(trial: Path) -> tuple[list[dict], list[dict]]:
    """Keep every session's outcome, including unusable earlier snapshots."""
    from harbor.models.task.task import Task
    from .candidate_capture import VERSION

    trial = trial.resolve()
    result = json.loads((trial / "result.json").read_text())
    if not result.get("finished_at") or replay.original_reward(trial) is None:
        raise replay.ReplayError(f"{trial.name}: trial has not finished with a verifier result")
    task = replay.task_dir(trial).resolve()
    if not result.get("task_checksum") or Task(task).checksum != result["task_checksum"]:
        raise replay.ReplayError(f"{trial.name}: current task differs from its recorded checksum")
    task_files = inventory(task)
    rows, errors = [], []
    seen = set()
    for selection in sorted(trial.glob("agent/episode/artifacts/lean-*/selection.json")):
        try:
            moves = json.loads(selection.read_text())
            if not isinstance(moves, list):
                raise ValueError("selection must contain an array")
        except (OSError, ValueError) as error:
            errors.append({"trial": str(trial), "dispatch": selection.parent.name, "error": str(error)})
            continue
        for move in moves:
            if not isinstance(move, dict):
                errors.append({"trial": str(trial), "dispatch": selection.parent.name, "error": "invalid selection entry"})
                continue
            if move.get("kind") != "lean":
                continue
            number = move.get("after_session")
            try:
                if type(number) is not int or number < 1 or (selection.parent.name, number) in seen:
                    raise replay.ReplayError("invalid or duplicate candidate session number")
                seen.add((selection.parent.name, number))
                name = f"session-{number}"
                candidate = selection.parent / name
                capture = move.get("capture")
                capture_dir = trial / "agent/candidate-checkpoints" / f"{selection.parent.name}-{name}"
                if capture:
                    receipt = json.loads((capture_dir / "receipt.json").read_text())
                    if (receipt.get("schema") != VERSION or not receipt.get("complete")
                            or receipt.get("verifier_executed") is not False
                            or digest(receipt) != capture.get("receipt_digest")
                            or receipt.get("id") != capture_dir.name
                            or receipt["coverage"]["task_checksum"] != result["task_checksum"]):
                        raise replay.ReplayError("checkpoint receipt identity is inconsistent")
                    files = inventory(capture_dir / "artifacts")
                    if files != receipt["files"]:
                        raise replay.ReplayError("checkpoint artifacts changed after collection")
                    inputs = {"task": task_files, "artifacts": files, "receipt_digest": digest(receipt)}
                else:
                    if not move.get("candidate") or Path(move["candidate"]).name != name or move.get("snapshot_error"):
                        raise replay.ReplayError("snapshot unavailable: " + str(move.get("snapshot_error") or "not recorded"))
                    files = inventory(candidate)
                    if not isinstance(move.get("workspace_files"), dict) or recorded_files(files) != move["workspace_files"]:
                        raise replay.ReplayError("candidate does not match its retained identity")
                    mount = replay.workdir_of(trial)
                    if not mount.startswith("/") or ".." in Path(mount).parts or mount == "/":
                        raise replay.ReplayError(f"unsupported candidate mount: {mount}")
                    inputs = {"task": task_files, "candidate": files, "mount": mount}
                rows.append({"trial": str(trial), "task": str(task), "candidate": str(candidate),
                             "capture_dir": str(capture_dir) if capture else None,
                             "session": number, "dispatch": selection.parent.name,
                             "original_reward": replay.original_reward(trial), "self_score": move.get("score"),
                             "kept": move.get("kept"), "input_digest": digest(inputs), "inputs": inputs})
            except (OSError, ValueError, TypeError, KeyError, replay.ReplayError) as error:
                errors.append({"trial": str(trial), "dispatch": selection.parent.name,
                               "session": number, "error": str(error)})
    from .candidate_capture import IDENTITY
    orphaned = set()
    for checkpoint in sorted((trial / "agent/candidate-checkpoints").glob("lean-*-session-*")):
        if IDENTITY.fullmatch(checkpoint.name):
            dispatch, number = checkpoint.name.rsplit("-session-", 1)
            if (dispatch, int(number)) not in seen:
                orphaned.add((dispatch, int(number)))
                errors.append({"trial": str(trial), "dispatch": dispatch, "session": int(number),
                               "error": "checkpoint has no retained executor acknowledgement; coverage is unknown"})
    for trace in trial.glob("agent/episode/artifacts/microluna-*-*.atif.jsonl"):
        match = re.fullmatch(r"microluna-([1-9][0-9]*)-([1-9][0-9]*)\.atif\.jsonl", trace.name)
        if match:
            identity = (f"lean-{match[1]}", int(match[2]))
            if identity not in seen and identity not in orphaned:
                errors.append({"trial": str(trial), "dispatch": identity[0], "session": identity[1],
                               "error": "executor session has no retained candidate; coverage is unknown"})
    if not rows and not errors:
        raise replay.ReplayError(f"{trial.name}: no retained sequential candidates")
    return rows, errors


def discover(trial: Path) -> list[dict]:
    """Strict discovery for callers that require every candidate to be usable."""
    rows, errors = discover_with_errors(trial)
    if errors:
        raise replay.ReplayError(errors[0]["error"])
    return rows


def valid_grade(grade: dict) -> bool:
    reward = grade.get("reward")
    return (grade.get("exit") == 0 and not grade.get("exception")
            and type(reward) in (int, float) and math.isfinite(reward) and reward in (0, 1))


def batch(trials: list[Path], out: Path, *, jobs: int = 2, deduplicate: bool = False,
          runner: Callable = replay.run_verifier) -> dict:
    """Write every outcome, preserving failures and explicit grade reuse."""
    if not 1 <= jobs <= 8:
        raise replay.ReplayError("candidate grading requires 1 to 8 workers")
    out = out.resolve()
    for trial in trials:
        if out.is_relative_to(trial.resolve()):
            raise replay.ReplayError("grading output must stay outside the trial")
    for trial in trials:
        if out.is_relative_to(replay.task_dir(trial).resolve()):
            raise replay.ReplayError("grading output must stay outside the task")
    out.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    rows = []
    errors = []
    for trial in dict.fromkeys(trials):
        try:
            found, failed = discover_with_errors(trial)
            rows.extend(found)
            errors.extend(failed)
        except (OSError, ValueError, replay.ReplayError) as error:
            errors.append({"trial": str(trial), "error": str(error)})
    groups = {}
    for index, row in enumerate(rows):
        row["id"] = f"candidate-{index + 1:04d}"
        key = row["input_digest"] if deduplicate else row["id"]
        groups.setdefault(key, []).append(row)

    def evaluate(group):
        reusable = None
        for row in group:
            dest = out / row["id"]
            dest.mkdir()
            before = time.monotonic()
            row["reused_from"] = None
            row["verifier_executed"] = False
            try:
                candidate, task = Path(row["candidate"]), Path(row["task"])
                capture_dir = Path(row["capture_dir"]) if row.get("capture_dir") else None
                current = inventory(capture_dir / "artifacts") if capture_dir else inventory(candidate)
                expected = row["inputs"].get("artifacts", row["inputs"].get("candidate"))
                if current != expected or inventory(task) != row["inputs"]["task"]:
                    raise replay.ReplayError("input changed after discovery")
                if reusable:
                    row["grade"] = reusable["grade"]
                    row["reused_from"] = reusable["id"]
                else:
                    with tempfile.TemporaryDirectory(prefix="tbench-candidate-") as temporary:
                        scratch = Path(temporary)
                        frozen_task = scratch / "tasks" / task.name
                        shutil.copytree(task, frozen_task)
                        if capture_dir:
                            frozen_capture = scratch / "capture"
                            shutil.copytree(capture_dir, frozen_capture)
                            if (inventory(frozen_capture / "artifacts") != expected
                                    or digest(json.loads((frozen_capture / "receipt.json").read_text()))
                                    != row["inputs"]["receipt_digest"]):
                                raise replay.ReplayError("checkpoint changed while copying")
                            workspace = replay.Workspace(scratch / "root", "checkpoint", False,
                                                         "sealed per-session artifacts", frozen_capture)
                        else:
                            frozen_candidate = scratch / "candidate"
                            shutil.copytree(candidate, frozen_candidate)
                            if inventory(frozen_candidate) != expected:
                                raise replay.ReplayError("candidate copy differs from its identity")
                            workspace = replay.candidate_workspace(frozen_candidate, row["inputs"]["mount"], scratch / "workspace")
                        if inventory(frozen_task) != row["inputs"]["task"]:
                            raise replay.ReplayError("task changed while copying")
                        row["verifier_executed"] = True
                        row["grade"] = runner(frozen_task, workspace, dest)
                    if valid_grade(row["grade"]):
                        reusable = row
                if not valid_grade(row["grade"]):
                    row["error"] = "verifier did not return a successful, complete binary grade"
            except Exception as error:
                row["error"] = str(error)
            row["seconds"] = time.monotonic() - before
            (dest / "candidate.json").write_text(json.dumps(row, indent=2) + "\n")
        return group

    with ThreadPoolExecutor(max_workers=jobs) as pool:
        list(pool.map(evaluate, groups.values()))
    oracle = []
    for trial in dict.fromkeys(str(t.resolve()) for t in trials):
        group = [row for row in rows if row["trial"] == trial]
        discovery_errors = sum(str(Path(e["trial"]).resolve()) == trial for e in errors)
        complete = bool(group) and not discovery_errors and all("error" not in row for row in group)
        passes = any(row.get("grade", {}).get("reward") == 1 and "error" not in row for row in group)
        oracle.append({"trial": trial, "original_reward": replay.original_reward(Path(trial)),
                       "candidates": len(group), "discovery_errors": discovery_errors, "complete": complete,
                       "any_candidate_passes": True if passes else (False if complete else None)})
    result = {"schema": SCHEMA, "jobs": jobs, "deduplicate": deduplicate,
              "deduplication_scope": "this batch only; assumes deterministic verifier and unchanged Docker environment",
              "wall_seconds": time.monotonic() - started, "errors": errors,
              "candidates": rows, "oracle": oracle,
              "verifier_executions": sum(row["verifier_executed"] for row in rows),
              "reused_grades": sum(bool(row["reused_from"]) for row in rows),
              "invalid_candidates": sum("error" in row for row in rows)}
    (out / "batch.json").write_text(json.dumps(result, indent=2) + "\n")
    return result
