#!/usr/bin/env python3
# A stand-in for /chat's engine (chat.coax) in the runner's checks
# (coax/web/api/test_zchat.py): it speaks the same JSON lines without a
# model and answers with what it was given, so the checks can see which
# credential the turn got (a short hash, never the value), whether it resumed
# its session, the model, and a file from its checkout. "slow" waits for STOP;
# "costly" costs a dollar; "expire" says the subscription token expired. It
# writes the session file where the CLI would. A project turn (no session,
# no README.md) gets the same answer. Tooling only.
import hashlib, json, os, sys

def send(x):
    sys.stdout.write(json.dumps(x) + "\n"); sys.stdout.flush()

first = json.loads(sys.stdin.readline())
text = first["turn"]["text"]
sid = first.get("resume") or first.get("session_id") or "project"
send({"t": "event", "type": "working", "data": {"text": "reading README.md"}})
send({"t": "event", "type": "agent.tool_use", "data": {"name": "Read", "detail": "README.md"}})
if "slow" in text:
    while True:
        line = sys.stdin.readline()
        if not line or json.loads(line).get("t") == "stop":
            send({"t": "done", "status": "stopped", "answer": "", "error": "", "cost_usd": 0.01})
            sys.exit(0)
creds = [k for k in ("ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_AUTH_TOKEN") if os.environ.get(k)]
cred = ",".join("%s:%s" % (k, hashlib.sha256(os.environ[k].encode()).hexdigest()[:8]) for k in creds)
cfg = os.environ.get("CLAUDE_CONFIG_DIR", "")
real = os.path.realpath(first["cwd"])
proj = os.path.join(cfg, "projects", "".join(c if c.isalnum() else "-" for c in real))
os.makedirs(proj, exist_ok=True)
path = os.path.join(proj, sid + ".jsonl")
turns = sum(1 for _ in open(path)) if os.path.exists(path) else 0
with open(path, "a") as f:
    f.write(json.dumps({"said": len(text)}) + "\n")
readme_path = os.path.join(first["cwd"], "README.md")
readme = open(readme_path).read().strip() if os.path.exists(readme_path) else "none"
others = sorted(k for k in os.environ if k not in ("HOME", "PATH", "CLAUDE_CONFIG_DIR", "OA_AGENT_MODEL", "DISABLE_AUTOUPDATER",
                                                     "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "ANTHROPIC_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN", "OA_AGENT_SLUG", "PWD", "SHLVL", "_", "LC_CTYPE", "__CF_USER_TEXT_ENCODING")
                 # macOS's /usr/bin/python3 shim adds these itself
                 and k not in ("CPATH", "LIBRARY_PATH", "MANPATH", "SDKROOT"))
answer = "cred=%s resume=%s turns=%d model=%s mode=%s readme=%s budget=%.2f others=%s" % (
    cred, "yes" if first.get("resume") else "no", turns, first["model"], first.get("workspace", {}).get("kind"), readme, first["budget_usd"], "+".join(others) or "none")
if "expire" in text:
    send({"t": "done", "status": "failed", "answer": "", "error": "Your token expired. Make a new one with claude setup-token and save it under KEY.", "credential_expired": True, "cost_usd": 0})
else:
    send({"t": "done", "status": "done", "answer": "## Answer\n\n" + answer, "error": "", "credential_expired": False, "cost_usd": 1.0 if "costly" in text else 0.02})
