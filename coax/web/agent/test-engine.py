#!/usr/bin/env python3
# A stand-in engine for the runner's checks (coax/web/api/test_private_runner.py):
# it speaks the runner's JSON lines like the Coax SDK engine does, without a
# model. It reads the project and moves a milestone with the project tools,
# edits a file in its checkout,
# and answers; a message with "slow" in it waits for STOP. Tooling only.
import json, os, sys

def send(x):
    sys.stdout.write(json.dumps(x) + "\n"); sys.stdout.flush()

def call(k, tool, args=None):
    send({"t": "call", "id": k, "tool": tool, "args": args or {}})
    while True:
        m = json.loads(sys.stdin.readline())
        if m.get("t") == "reply" and m["id"] == k:
            return m

first = json.loads(sys.stdin.readline())
turn = first["turn"]
send({"t": "event", "type": "working", "data": {"text": "reading the milestones"}})
if "slow" in turn["text"]:
    while True:
        line = sys.stdin.readline()
        if not line or json.loads(line).get("t") == "stop":
            send({"t": "done", "status": "stopped", "answer": "", "error": "", "cost_usd": 0.01})
            sys.exit(0)
state = call(1, "project_get")
version = state["body"]["version"]
r = call(2, "milestone_status", {"id": "m1", "status": "done", "evidence": "the deck is in the repo"})
send({"t": "event", "type": "agent.tool_use", "data": {"name": "Edit", "detail": "project/plan.md"}})
os.makedirs("project", exist_ok=True)
with open("project/plan.md", "a") as f:
    f.write("- the deck is done\n")
seen = "key:%s runner:%s home:%s" % ("yes" if os.environ.get("ANTHROPIC_API_KEY") else "no",
                                     "yes" if os.environ.get("OA_RUNNER_KEY") else "no", "yes" if os.environ.get("HOME") else "no")
send({"t": "done", "status": "done", "answer": "Marked it done (status %d, from version %d, in %s). %s" % (r["status"], version, "cwd" if first["cwd"] == os.getcwd() else "elsewhere", seen), "error": "", "cost_usd": 0.05})
