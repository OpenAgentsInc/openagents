#!/usr/bin/env python3
"""A stand-in `claude` for the smoke: speaks enough of Claude Code's
stream-json control protocol to ask for two permissions, a Write inside the
working directory and a Bash command, and records each answer.

CLAUDE_STUB_OUT names the file the answers go to, one JSON line each.
CLAUDE_STUB_MODE=mcp calls Coder's in-process coder_ls tool instead, and
CLAUDE_STUB_MODE=mcprun starts a child session with coder_run, and
CLAUDE_STUB_MODE=hook sends two Bash calls through the PreToolUse hook."""
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
hook_id = None
while True:
    m = read()
    if m.get("type") == "control_request" and m["request"].get("subtype") == "initialize":
        pre = (m["request"].get("hooks") or {}).get("PreToolUse") or []
        if pre and pre[0].get("hookCallbackIds"):
            hook_id = pre[0]["hookCallbackIds"][0]
        send({"type": "control_response", "response": {"subtype": "success", "request_id": m["request_id"], "response": {"commands": [], "models": []}}})
        continue
    if m.get("type") == "user":
        break
send({"type": "system", "subtype": "init", "session_id": "stub", "cwd": cwd, "tools": [], "model": "stub"})
mode = os.environ.get("CLAUDE_STUB_MODE", "")
if mode == "hook":
    # two tool calls through the host's PreToolUse hook; record each decision
    with open(out, "a") as f:
        for i, cmd in enumerate(["ls", "rm -rf build"]):
            send({"type": "control_request", "request_id": "stub-hook-%d" % i, "request": {"subtype": "hook_callback", "callback_id": hook_id or "none",
                  "input": {"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": cmd}}, "tool_use_id": "toolu_h%d" % i}})
            r = read()
            while r.get("type") != "control_response":
                r = read()
            got = r["response"].get("response", {}) or {}
            f.write(json.dumps({"command": cmd, "decision": (got.get("hookSpecificOutput") or {}).get("permissionDecision", "allow"), "hook_id": hook_id}) + "\n")
    asks = []
if mode in ("mcp", "mcprun"):
    # call Coder's in-process MCP server: coder_ls, or coder_run of a child task; record the answer
    call = {"name": "coder_ls", "arguments": {}} if mode == "mcp" else {"name": "coder_run", "arguments": {"task": "child task", "engine": "echo"}}
    send({"type": "control_request", "request_id": "stub-mcp", "request": {"subtype": "mcp_message", "server_name": "coder",
          "message": {"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": call}}})
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
