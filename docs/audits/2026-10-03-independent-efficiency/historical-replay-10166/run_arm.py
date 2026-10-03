#!/usr/bin/env python3
"""Run one bounded Claude replay. Raw event streams must remain private."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--prompt", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    workspace = args.workspace.resolve(strict=True)
    output = args.output.resolve()
    if output == workspace or workspace in output.parents:
        parser.error("Keep run artifacts outside the executor workspace.")
    prompt = args.prompt.read_bytes()
    output.mkdir(mode=0o700, exist_ok=False)
    command = [
        "claude", "-p", "--safe-mode", "--restricted",
        "--tools", "Read,Edit,Write,Glob,Grep",
        "--permission-mode", "acceptEdits", "--permission-prompts", "none",
        "--strict-mcp-config", "--mcp-config", '{"mcpServers":{}}',
        "--disable-slash-commands", "--no-chrome", "--no-session-persistence",
        "--output-format", "stream-json", "--verbose",
        "--model", "claude-opus-5-5", "--effort", "medium",
        "--max-budget-usd", "10",
    ]
    metadata = {
        "started_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "command": command,
        "prompt_sha256": hashlib.sha256(prompt).hexdigest(),
        "prompt_bytes": len(prompt),
        "workspace": str(workspace),
    }
    save(output / "request.json", metadata)
    started = time.monotonic()
    with (output / "events.jsonl").open("wb") as stdout, (output / "stderr.log").open("wb") as stderr:
        process = subprocess.Popen(
            command, cwd=workspace, stdin=subprocess.PIPE, stdout=stdout,
            stderr=stderr, start_new_session=True,
        )
        try:
            process.communicate(prompt, timeout=600)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            metadata["timeout"] = True
    metadata["exit_code"] = process.returncode
    metadata["wall_s"] = time.monotonic() - started
    for line in (output / "events.jsonl").read_text().splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("type") == "result":
            metadata["result"] = event
        if event.get("type") == "system" and event.get("subtype") == "init":
            metadata["init"] = event
    save(output / "result.json", metadata)
    result = metadata.get("result", {})
    print(json.dumps({
        "output": str(output), "exit_code": metadata["exit_code"],
        "wall_s": metadata["wall_s"], "result_type": result.get("subtype"),
        "cost_usd": result.get("total_cost_usd"), "turns": result.get("num_turns"),
        "models": list(result.get("modelUsage", {})),
    }))


if __name__ == "__main__":
    main()
