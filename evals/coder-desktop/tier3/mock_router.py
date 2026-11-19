#!/usr/bin/env python3
# Firm reason: 5 (picks the Tier 1 mock worker's script for each worker a scratch Coder.app starts)
"""Tier 3 worker stand-in.

The Tier 3 runner puts a two-line `mcworker` in the scratch Coder.app's Resources that runs this
file. Every worker Coder starts (the chat's own and each sub-agent's) is this process. It reads
lines until the first {"task": ...} line, picks a script from the flow's worker file
(CODER_T3_WORKERS) and hands the rest to evals/coder-desktop/mock_worker/mock_worker.py:

  {"main": [{"match": "TEXT", "script": {...}}, ...],     first whose TEXT is in the task text
   "default": {...},                                         when none matches
   "agents": {"0": {...}, "1": {...}}}                       sub-agent J (session has -agent-J-)

Every line from Coder is recorded to CODER_T3_RECORD/<role>-<pid>.jsonl.
"""

from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "mock_worker"))
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


def main() -> int:
    spec = json.loads(Path(os.environ["CODER_T3_WORKERS"]).read_text(encoding="utf-8"))
    record_dir = Path(os.environ.get("CODER_T3_RECORD", "."))
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
