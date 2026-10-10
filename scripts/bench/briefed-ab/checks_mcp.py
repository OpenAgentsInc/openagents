#!/usr/bin/env python3
"""The briefed agent's only command tool (#11211): an MCP stdio server with
one tool, `run_check`, that runs a check the briefing names and nothing
else.

Env: AB_CHECKS (the briefing's checks as JSON), AB_CHECK_OUTPUT (`smart`
keeps the compiler errors and failing tests, cut short; `full` keeps the
tail), plus remote-exec's AB_ROOT, AB_BASE and AB_SLOT. Every run is
appended to AB_CHECK_LOG when set.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
CHECKS = json.loads(os.environ.get("AB_CHECKS", "[]"))
MODE = os.environ.get("AB_CHECK_OUTPUT", "smart")
LOG = os.environ.get("AB_CHECK_LOG")
LIMIT = 7000


def smart(text: str, ok: bool) -> str:
    lines = text.splitlines()
    if ok:
        keep = [l for l in lines if l.startswith("test result:") or l.startswith("warning: unused")]
        summary = "\n".join(keep[-12:]) or (lines[-1] if lines else "")
        return "ok\n" + summary
    out, block = [], False
    for line in lines:
        if re.match(r"^(error|warning: unused)", line) or line.startswith("---- ") or "panicked at" in line:
            block = True
        elif line.startswith("test result:") or line.startswith("failures:"):
            out.append(line)
            block = False
            continue
        elif block and line.strip() == "":
            out.append("")
            block = False
            continue
        if block:
            out.append(line)
    body = "\n".join(out).strip() or "\n".join(lines[-60:])
    if len(body) > LIMIT:
        body = body[:LIMIT] + "\n... (cut; fix the first errors and run again)"
    return body


def run_check(check_id: str, test_filter: str | None) -> tuple[str, bool]:
    check = next((c for c in CHECKS if c["id"] == check_id), None)
    if check is None:
        return "Unknown check. Available: " + ", ".join(c["id"] for c in CHECKS), False
    argv = list(check["argv"])
    if test_filter and check.get("filter"):
        words = [w for w in test_filter.split() if re.fullmatch(r"[A-Za-z0-9_:]+", w)][:8]
        argv += ["--", *words]
    started = time.time()
    proc = subprocess.run(
        [str(HERE / "shim" / "remote-exec"), *argv],
        cwd=os.environ["AB_ROOT"], capture_output=True, text=True,
    )
    ok = proc.returncode == 0
    text = proc.stdout + proc.stderr
    if LOG:
        with open(LOG, "a") as f:
            f.write(json.dumps({"check": check_id, "filter": test_filter, "ok": ok,
                                "secs": round(time.time() - started, 1), "chars": len(text)}) + "\n")
    if MODE == "full":
        body = text[-LIMIT:]
    else:
        body = smart(text, ok)
    return (f"{' '.join(argv)}: {'passed' if ok else 'FAILED'}\n{body}"), ok


def tool_spec() -> dict:
    return {
        "name": "run_check",
        "description": "Run one of the briefing's checks on the build host against your working copy. "
        "Checks: " + "; ".join(f"{c['id']} ({c['what']})" for c in CHECKS),
        "inputSchema": {
            "type": "object",
            "properties": {
                "check": {"type": "string", "enum": [c["id"] for c in CHECKS]},
                "filter": {"type": "string", "description": "test-name filter words (test checks only)"},
            },
            "required": ["check"],
        },
    }


def main() -> None:
    for raw in sys.stdin:
        raw = raw.strip()
        if not raw:
            continue
        msg = json.loads(raw)
        method, mid = msg.get("method"), msg.get("id")
        if mid is None:
            continue
        if method == "initialize":
            result = {
                "protocolVersion": msg.get("params", {}).get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "checks", "version": "1"},
            }
        elif method == "tools/list":
            result = {"tools": [tool_spec()]}
        elif method == "tools/call":
            args = msg["params"].get("arguments", {})
            text, ok = run_check(args.get("check", ""), args.get("filter"))
            result = {"content": [{"type": "text", "text": text}], "isError": not ok}
        elif method == "ping":
            result = {}
        else:
            sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "unknown method"}}) + "\n")
            sys.stdout.flush()
            continue
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": mid, "result": result}) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
