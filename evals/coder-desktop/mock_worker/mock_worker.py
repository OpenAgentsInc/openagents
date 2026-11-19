#!/usr/bin/env python3
# Firm reason: 5 (deterministic mock worker script for Tier 1 Coder Desktop evals)
"""Deterministic mock worker speaking mcworker-wire JSONL over stdin/stdout.

Replays a JSON script and records every owner->worker line (mww_task,
mww_answer, mww_results, MWW_STOP_LINE, MWW_RECALL, MWW_FORGET_LINE) to
--record PATH as JSONL for test assertions.

Script schema (JSON object):
{
  "turns": [
    {
      "steps": [
        {"emit": {"kind": "reply", "text": "hello"}},
        {"emit_many": [{...}, {...}]},
        {"repeat": {"count": 100, "template": {"kind": "command", "command": "cmd {i}", "exit": 0, "timed_out": false, "output": "ok"}}},
        {"repeat_ask": {"count": 101, "command_template": "ls {i}", "ticket_start": 1}},
        {"expect": "answer"},
        {"expect": "results"},
        {"wait_for_stop": true, "then_emit": {"kind": "ended", "ending": "stopped", "text": ""}},
        {"crash": 1, "stderr": "simulated crash"},
        {"eof": true}
      ]
    }
  ]
}

Each turn waits for an owner->worker {"task": ...} line on stdin. Any
{"recall": ...} or {"forget": true} lines received before a task are recorded
and skipped cleanly without consuming a turn.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from typing import Any


def classify_owner_line(obj: dict[str, Any]) -> str:
    if "task" in obj:
        return "task"
    if "answer" in obj:
        return "answer"
    if "results" in obj:
        return "results"
    if obj.get("stop") is True:
        return "stop"
    if "recall" in obj:
        return "recall"
    if obj.get("forget") is True:
        return "forget"
    if "steer" in obj:
        return "steer"
    if "interrupt" in obj:
        return "interrupt"
    if "queue" in obj:
        return "queue"
    return "unknown"


class MockWorker:
    def __init__(self, script: dict[str, Any], record_path: str | None) -> None:
        self.script = script
        self.record_fp = open(record_path, "a", encoding="utf-8") if record_path else None
        self.pending_lines: list[dict[str, Any]] = []

    def close(self) -> None:
        if self.record_fp is not None:
            self.record_fp.flush()
            self.record_fp.close()
            self.record_fp = None

    def record(self, raw_line: str, parsed: dict[str, Any] | None) -> None:
        if self.record_fp is None:
            return
        entry: dict[str, Any] = {
            "raw": raw_line,
            "kind": classify_owner_line(parsed) if isinstance(parsed, dict) else "invalid",
        }
        if isinstance(parsed, dict):
            entry["msg"] = parsed
        self.record_fp.write(json.dumps(entry, separators=(",", ":")) + "\n")
        self.record_fp.flush()

    def read_owner_line(self) -> dict[str, Any] | None:
        if self.pending_lines:
            return self.pending_lines.pop(0)
        while True:
            line = sys.stdin.readline()
            if not line:
                return None
            stripped = line.rstrip("\r\n")
            if not stripped:
                continue
            try:
                parsed = json.loads(stripped)
            except json.JSONDecodeError:
                self.record(stripped, None)
                continue
            self.record(stripped, parsed)
            return parsed

    def emit(self, obj: dict[str, Any]) -> None:
        sys.stdout.write(json.dumps(obj, separators=(",", ":")) + "\n")
        sys.stdout.flush()

    def format_template(self, obj: Any, i: int) -> Any:
        if isinstance(obj, str):
            return obj.replace("{i}", str(i))
        if isinstance(obj, list):
            return [self.format_template(x, i) for x in obj]
        if isinstance(obj, dict):
            return {k: self.format_template(v, i) for k, v in obj.items()}
        return obj

    def run_steps(self, steps: list[dict[str, Any]]) -> bool:
        """Execute scripted steps. Returns False if worker should exit immediately."""
        for step in steps:
            if "sleep_ms" in step:
                time.sleep(float(step["sleep_ms"]) / 1000.0)

            if "emit_raw" in step:
                sys.stdout.write(str(step["emit_raw"]) + "\n")
                sys.stdout.flush()

            if "emit" in step:
                self.emit(step["emit"])

            if "emit_many" in step:
                for item in step["emit_many"]:
                    self.emit(item)

            if "repeat" in step:
                rep = step["repeat"]
                count = int(rep["count"])
                tmpl = rep["template"]
                if isinstance(tmpl, list):
                    for i in range(count):
                        for item in tmpl:
                            self.emit(self.format_template(item, i))
                else:
                    for i in range(count):
                        self.emit(self.format_template(tmpl, i))

            if "repeat_ask" in step:
                ra = step["repeat_ask"]
                count = int(ra["count"])
                cmd_tmpl = str(ra.get("command_template", "cmd {i}"))
                ticket = int(ra.get("ticket_start", 1))
                emit_cmd = bool(ra.get("emit_command_after_allow", False))
                on_deny_continue = bool(ra.get("on_deny_continue", False))
                on_deny_emit = ra.get("on_deny_emit")
                denied = False
                for i in range(count):
                    cmd_str = cmd_tmpl.replace("{i}", str(i))
                    self.emit({"kind": "ask", "command": cmd_str, "ticket": ticket + i})
                    while True:
                        msg = self.read_owner_line()
                        if msg is None:
                            return False
                        kind = classify_owner_line(msg)
                        if kind == "answer":
                            if msg.get("answer") is False:
                                if on_deny_emit:
                                    self.emit(on_deny_emit)
                                if on_deny_continue:
                                    denied = True
                                    break
                                if not on_deny_emit:
                                    self.emit({"kind": "ended", "ending": "stopped", "text": "denied"})
                                return True
                            if emit_cmd:
                                self.emit({"kind": "command", "command": cmd_str, "exit": 0, "timed_out": False, "output": "ok"})
                            break
                        if kind == "stop":
                            self.emit({"kind": "ended", "ending": "stopped", "text": ""})
                            return True
                    if denied:
                        break

            if "expect" in step:
                expected = step["expect"]
                while True:
                    msg = self.read_owner_line()
                    if msg is None:
                        return False
                    kind = classify_owner_line(msg)
                    if expected == "any" or kind == expected:
                        if kind == "answer" and msg.get("answer") is False and step.get("on_deny_emit"):
                            self.emit(step["on_deny_emit"])
                            return True
                        break
                    if kind == "stop":
                        # Allow stop to interrupt an expectation cleanly.
                        self.emit({"kind": "ended", "ending": "stopped", "text": ""})
                        return True

            if step.get("wait_for_stop"):
                while True:
                    msg = self.read_owner_line()
                    if msg is None:
                        return False
                    if classify_owner_line(msg) == "stop":
                        if "then_emit" in step:
                            self.emit(step["then_emit"])
                        break

            if "crash" in step:
                if "stderr" in step:
                    sys.stderr.write(str(step["stderr"]) + "\n")
                    sys.stderr.flush()
                self.close()
                sys.exit(int(step["crash"]))

            if step.get("eof"):
                self.close()
                sys.exit(0)

        return True

    def run(self) -> int:
        turns = self.script.get("turns", [])
        turn_idx = 0
        while True:
            msg = self.read_owner_line()
            if msg is None:
                return 0
            kind = classify_owner_line(msg)
            if kind in ("recall", "forget"):
                continue
            if kind == "stop":
                # Stop received while idle; ignore.
                continue
            if kind != "task":
                continue
            if turn_idx >= len(turns):
                self.emit({"kind": "ended", "ending": "finished", "text": "mock worker: no more scripted turns"})
                continue
            turn = turns[turn_idx]
            turn_idx += 1
            if not self.run_steps(turn.get("steps", [])):
                return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Deterministic mcworker-wire mock worker")
    parser.add_argument("--script", required=True, help="Path to JSON scenario script")
    parser.add_argument("--record", default=None, help="Path to JSONL file recording owner->worker lines")
    args = parser.parse_args()

    with open(args.script, "r", encoding="utf-8") as f:
        script = json.load(f)

    worker = MockWorker(script, args.record)
    try:
        return worker.run()
    finally:
        worker.close()


if __name__ == "__main__":
    sys.exit(main())
