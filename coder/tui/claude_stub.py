#!/usr/bin/env python3
"""A stand-in `claude` for the smoke: speaks enough of Claude Code's
stream-json control protocol to ask for two permissions, a Write inside the
working directory and a Bash command, and records each answer.

CLAUDE_STUB_OUT names the file the answers go to, one JSON line each.
CLAUDE_STUB_MODE=mcp calls Coder's in-process coder_ls tool instead."""
import json, os, sys

out = os.environ.get("CLAUDE_STUB_OUT", "/dev/null")
cwd = os.getcwd()


def send(obj):
    sys.stdout.write(json.dumps(obj, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def read():
    while True:
        line = sys.stdin.readline()
        if not line:
            sys.exit(0)
        line = line.strip()
        if line:
            return json.loads(line)


asks = [
    ("Write", {"file_path": os.path.join(cwd, "a.txt"), "content": "hi"}),
    ("Write", {"file_path": "/etc/stub-outside.txt", "content": "hi"}),
    ("Bash", {"command": "touch b.txt"}),
]
while True:
    m = read()
    if m.get("type") == "control_request" and m["request"].get("subtype") == "initialize":
        send({"type": "control_response", "response": {"subtype": "success", "request_id": m["request_id"], "response": {"commands": [], "models": []}}})
        continue
    if m.get("type") == "user":
        break
send({"type": "system", "subtype": "init", "session_id": "stub", "cwd": cwd, "tools": [], "model": "stub"})
if os.environ.get("CLAUDE_STUB_MODE") == "mcp":
    # call Coder's in-process MCP server's coder_ls, and record its text
    send({"type": "control_request", "request_id": "stub-mcp", "request": {"subtype": "mcp_message", "server_name": "coder",
          "message": {"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "coder_ls", "arguments": {}}}}})
    r = read()
    while r.get("type") != "control_response":
        r = read()
    with open(out, "a") as f:
        f.write(json.dumps({"mcp": r["response"].get("response", {}).get("mcp_response")}) + "\n")
    asks = []
with open(out, "a") as f:
    for i, (tool, inp) in enumerate(asks):
        send({"type": "control_request", "request_id": "stub-%d" % i, "request": {"subtype": "can_use_tool", "tool_name": tool, "input": inp, "tool_use_id": "toolu_stub%d" % i}})
        r = read()
        while r.get("type") != "control_response":
            r = read()
        f.write(json.dumps({"tool": tool, "behavior": r["response"].get("response", {}).get("behavior")}) + "\n")
send({"type": "result", "subtype": "success", "is_error": False, "result": "stub-done", "uuid": "u-result", "session_id": "stub",
      "num_turns": 1, "duration_ms": 1, "duration_api_ms": 1, "total_cost_usd": 0.0,
      "usage": {"input_tokens": 1, "output_tokens": 1}, "modelUsage": {}, "permission_denials": []})
read()
