#!/usr/bin/env python3
"""Fake Cursor SDK shim for keeper tests (crates/harness/tests/keeper_drivers.rs).

The opening `run` op whose prompt carries `chunks=N` streams N slow chunks and
finishes the turn; each `steer` answers `heard <text>`. A second `run` is a
protocol error: a driver that picks the run up must not start it again.
"""
import json
import re
import sys
import time


def emit(frame):
    sys.stdout.write(json.dumps(frame) + "\n")
    sys.stdout.flush()


if sys.argv[1:] == ["models"]:
    emit({"ev": "models", "items": []})
    sys.exit(0)

runs = 0
for line in sys.stdin:
    op = json.loads(line)
    if op["op"] == "run":
        runs += 1
        if runs > 1:
            sys.stderr.write("run again\n")
            sys.exit(2)
        emit({"ev": "ready", "agentId": "agent-kept", "model": "fake"})
        found = re.search(r"chunks=(\d+)", op["prompt"])
        for i in range(int(found.group(1)) if found else 0):
            emit({"ev": "text", "text": f"chunk {i}\n"})
            time.sleep(0.03)
        emit({"ev": "turn", "status": "finished"})
    elif op["op"] == "steer":
        emit({"ev": "steered"})
        emit({"ev": "text", "text": f"heard {op['prompt']}\n"})
        emit({"ev": "turn", "status": "finished"})
