#!/usr/bin/env python3
# Firm reason: 5 (picks the Tier 1 mock worker's script for each worker a scratch Coder.app starts)
"""Tier 3 worker stand-in.

The Tier 3 runner puts a two-line `mcworker` in the scratch Coder.app's Resources that runs this
file. Every worker Coder starts (the chat's own and each sub-agent's) is this process. It reads
lines until the first {"task": ...} line, picks a script from the flow's worker file
(CODER_T3_WORKERS) and hands the rest to evals/openagents-desktop/mock_worker/mock_worker.py:

  {"main": [{"match": "TEXT", "script": {...}}, ...],     first whose TEXT is in the task text
   "default": {...},                                         when none matches
   "agents": {"0": {...}, "1": {...}}}                       sub-agent J (session has -agent-J-)

  {"real": {"answers": [{"action": {...}, "think": "..."}, ...]}}

With "real", every main worker is the real worker (CODER_T3_REAL_WORKER, the bundle's own mcworker,
GitHub #13718): no script, the follow-up wire (steers, the queue, interrupts) as it really runs.
Nothing calls a model: GW_REPLAY points at CODER_T3_RECORD/replay, which holds the answers in order
as recorded responses. Every worker of the flow shares it, so a worker started later (Resume after a
relaunch) takes the next answer. The router starts it at once (Resume sends no task line) and only
passes Coder's lines on; the worker writes straight to Coder.

Every line from Coder is recorded to CODER_T3_RECORD/<role>-<pid>.jsonl. When the task line comes,
what the worker sees then goes to CODER_T3_RECORD/seen-<role>-<pid>.json: the task's folder, whether
it holds a .git (a file in a worktree) and the worker's CARGO_TARGET_DIR (GitHub #13771).
"""

from __future__ import annotations

import json
import os
import re
import signal
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "mock_worker"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "followups"))
import mock_worker  # noqa: E402

FINISHED = {"turns": []}


def pick(spec: dict, task: dict) -> tuple[str, dict]:
    m = re.search(r"-agent-(\d+)-", str(task.get("session", "")))
    if m:
        j = m.group(1)
        return f"agent-{j}", spec.get("agents", {}).get(j, FINISHED)
    text = str(task.get("task", ""))
    for route in spec.get("main", []):
        if route.get("match", "") in text:
            return "main", route["script"]
    return "main", spec.get("default", FINISHED)


def run_real(real: dict, record_dir: Path) -> int:
    from run_followups import http  # noqa: PLC0415 - only the real worker's flows need it
    replay = record_dir / "replay"
    replay.mkdir(exist_ok=True)
    for i, a in enumerate(real.get("answers", [])):
        f = replay / f"{i + 1}.http"
        if not f.exists():
            f.write_text(http(a["action"], a.get("think", "")), encoding="utf-8")
    env = dict(os.environ, GW_REPLAY=str(replay))
    env.pop("CODER_CLAUDE_KEY", None)
    child = subprocess.Popen([os.environ["CODER_T3_REAL_WORKER"]], stdin=subprocess.PIPE, env=env)
    signal.signal(signal.SIGTERM, lambda *_: (child.terminate(), sys.exit(0)))
    rec = mock_worker.MockWorker({"turns": []}, str(record_dir / f"main-{os.getpid()}.jsonl"))
    assert child.stdin is not None
    try:
        for raw in sys.stdin:
            line = raw.rstrip("\r\n")
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                obj = None
            rec.record(line, obj)
            try:
                child.stdin.write(raw.encode("utf-8")); child.stdin.flush()
            except BrokenPipeError:
                break
    finally:
        rec.close()
        try:
            child.stdin.close()
        except OSError:
            pass
    return child.wait()


def main() -> int:
    spec = json.loads(Path(os.environ["CODER_T3_WORKERS"]).read_text(encoding="utf-8"))
    record_dir = Path(os.environ.get("CODER_T3_RECORD", "."))
    if spec.get("real"):
        return run_real(spec["real"], record_dir)
    held: list[tuple[str, dict]] = []
    task = None
    while task is None:
        raw = sys.stdin.readline()
        if not raw:
            return 0
        raw = raw.rstrip("\r\n")
        if not raw:
            continue
        try:
            obj = json.loads(raw)
        except json.JSONDecodeError:
            continue
        held.append((raw, obj))
        if isinstance(obj, dict) and "task" in obj:
            task = obj
    role, script = pick(spec, task)
    cwd = str(task.get("cwd", ""))
    git = Path(cwd, ".git") if cwd else None
    seen = {"cwd": cwd, "git": "file" if git and git.is_file() else "dir" if git and git.is_dir() else "",
            "cargo_target_dir": os.environ.get("CARGO_TARGET_DIR", "")}
    (record_dir / f"seen-{role}-{os.getpid()}.json").write_text(json.dumps(seen), encoding="utf-8")
    worker = mock_worker.MockWorker(script, str(record_dir / f"{role}-{os.getpid()}.jsonl"))
    for raw, obj in held:
        worker.record(raw, obj)
    worker.pending_lines = [obj for _, obj in held]
    try:
        return worker.run()
    finally:
        worker.close()


if __name__ == "__main__":
    sys.exit(main())
