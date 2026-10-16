#!/usr/bin/env python3
"""Reproduce the 2026-08-12 stuck-Working incident against the real claude-agent-acp.

Evidence script for the upstream report: a session/prompt sent while the
Claude CLI runs a SELF-CONTINUED turn (background-task re-invocation) never
gets its response from @agentclientprotocol/claude-agent-acp@0.66.0, because
the adapter does not track turns it did not start (steering answers
`promptRequired`/`noRunningTurn` while the CLI is visibly busy). Zeron-side
mitigations: engine turn-quiesce watchdog + harness starved-turn recovery.

Needs an authenticated claude CLI; costs a few small prompts.

Speaks newline-delimited JSON-RPC 2.0 over the adapter's stdio, mirroring
zeron's harness. Timeline (mirrors the 2026-08-12 incident):

  1. initialize + session/new
  2. prompt#1: agent starts a background task (sleep) and ends its turn
     -> expect response#1 (stopReason) while the task is still running
  3. the task exits -> the CLI self-continues (a turn no prompt started);
     the agent is instructed to run a FOREGROUND sleep in that turn
  4. while that self-continued turn is busy, send prompt#2 ("what about now")
  5. watch whether response#2 EVER arrives

Every frame in both directions is logged with a monotonic timestamp to
/tmp/acp_repro/frames.log. Auto-approves any session/request_permission.
"""

import json
import os
import subprocess
import sys
import threading
import time

os.makedirs("/tmp/acp_repro/ws", exist_ok=True)
LOG = open("/tmp/acp_repro/frames.log", "a", buffering=1)
T0 = time.monotonic()


def log(direction, obj):
    t = time.monotonic() - T0
    line = json.dumps(obj, separators=(",", ":"))
    if len(line) > 600:
        line = line[:600] + f"...({len(line)}b)"
    print(f"{t:8.3f} {direction} {line}", file=LOG)


proc = subprocess.Popen(
    ["npx", "-y", "@agentclientprotocol/claude-agent-acp@0.66.0"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=open("/tmp/acp_repro/stderr.log", "ab"),
    text=True,
    bufsize=1,
    env={**os.environ},
)

next_id = 0
pending = {}          # id -> description
responses = {}        # id -> (t, payload)
lock = threading.Lock()
session_id = None
notif_count = 0


def send_request(method, params, desc):
    global next_id
    next_id += 1
    rid = next_id
    with lock:
        pending[rid] = desc
    msg = {"jsonrpc": "2.0", "id": rid, "method": method, "params": params}
    log(f">> req#{rid} [{desc}]", msg)
    proc.stdin.write(json.dumps(msg) + "\n")
    proc.stdin.flush()
    return rid


def respond(rid, result):
    msg = {"jsonrpc": "2.0", "id": rid, "result": result}
    log("<< resp(ours)", msg)
    proc.stdin.write(json.dumps(msg) + "\n")
    proc.stdin.flush()


def reader():
    global notif_count
    for raw in proc.stdout:
        raw = raw.strip()
        if not raw:
            continue
        try:
            msg = json.loads(raw)
        except json.JSONDecodeError:
            print(f"{time.monotonic()-T0:8.3f} !! non-json: {raw[:200]}", file=LOG)
            continue
        mid = msg.get("id")
        method = msg.get("method")
        if method is None and mid is not None:
            with lock:
                desc = pending.pop(mid, f"UNKNOWN-ID({mid})")
                responses[mid] = (time.monotonic() - T0, msg)
            log(f"** RESPONSE to #{mid} [{desc}]", msg)
        elif method is not None and mid is not None:
            log(f"?? server-req {method}", msg)
            if method == "session/request_permission":
                opts = (msg.get("params") or {}).get("options") or []
                allow = next(
                    (o for o in opts if "allow" in (o.get("kind") or "") or "allow" in (o.get("optionId") or "")),
                    opts[0] if opts else None,
                )
                outcome = {"outcome": {"outcome": "selected", "optionId": allow.get("optionId")}} if allow else {"outcome": {"outcome": "cancelled"}}
                respond(mid, outcome)
            else:
                respond(mid, {})
        else:
            notif_count += 1
            log(f".. notif {method}", msg)
    print(f"{time.monotonic()-T0:8.3f} !! adapter stdout EOF", file=LOG)


threading.Thread(target=reader, daemon=True).start()


def wait_response(rid, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        with lock:
            if rid in responses:
                return responses[rid]
        time.sleep(0.2)
    return None


# ---- 1. initialize + session/new -------------------------------------------
rid = send_request(
    "initialize",
    {
        "protocolVersion": 1,
        "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}},
    },
    "initialize",
)
assert wait_response(rid, 60), "initialize timed out"

rid = send_request(
    "session/new",
    {"cwd": "/tmp/acp_repro/ws", "mcpServers": []},
    "session/new",
)
resp = wait_response(rid, 120)
assert resp, "session/new timed out"
session_id = resp[1]["result"]["sessionId"]
print(f"session: {session_id}")

# ---- 2. prompt#1: background task, end turn ---------------------------------
PROMPT1 = (
    "Use the Bash tool exactly twice, then stop.\n"
    "First call: run the command `sleep 8; echo task-finished` with "
    "run_in_background set to true.\n"
    "Then reply with exactly the word: started\n"
    "IMPORTANT: later, when a task notification about that background task "
    "arrives, make one FOREGROUND Bash call: `sleep 20; echo waited` (no "
    "run_in_background), then reply with exactly: done waiting"
)
rid1 = send_request(
    "session/prompt",
    {"sessionId": session_id, "prompt": [{"type": "text", "text": PROMPT1}]},
    "PROMPT#1",
)
resp1 = wait_response(rid1, 240)
print(f"prompt#1 response: {json.dumps(resp1[1].get('result')) if resp1 else 'NEVER ARRIVED'}")
if not resp1:
    sys.exit(1)

# ---- 3. wait for self-continuation (task notification fires ~8s in) ---------
print("waiting 16s for the background task to exit and the CLI to self-continue...")
time.sleep(16)

# ---- 4. prompt#2 while the self-continued turn is busy ----------------------
rid2 = send_request(
    "session/prompt",
    {"sessionId": session_id, "prompt": [{"type": "text", "text": "what about now"}]},
    "PROMPT#2",
)
resp2 = wait_response(rid2, 180)
t = time.monotonic() - T0
if resp2:
    print(f"prompt#2 response at t={resp2[0]:.1f}s: {json.dumps(resp2[1].get('result'))}")
    print("VERDICT: adapter settled prompt#2 — no drop in this shape")
else:
    print(f"prompt#2 response NEVER ARRIVED (waited 180s, t={t:.1f}s)")
    print(f"adapter alive: {proc.poll() is None}, notifications seen: {notif_count}")
    print("VERDICT: REPRODUCED — the adapter/CLI lost the turn-end reply")

log("-- done --", {"prompt2_settled": bool(resp2)})
proc.terminate()
