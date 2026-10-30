#!/usr/bin/env python3
"""Records a `codex app-server` session for fixture.py: '>> ' lines sent,
'<< ' lines received. The script (JSON) names the thread params, the turns
and the approval answers. Uses the CLI's own login (no API key) and no browser.
    CODEX=/path/to/codex python3 record.py CWD OUT.log SCRIPT.json"""
import glob, json, os, shutil, subprocess, sys, time, select
codex = os.environ.get("CODEX") or shutil.which("codex") or sorted(glob.glob(os.path.expanduser("~/.nvm/versions/node/*/bin/codex")))[-1]
env = {k: v for k, v in os.environ.items() if k not in ("OPENAI_API_KEY", "CODEX_API_KEY")}
env["BROWSER"] = "/usr/bin/true"
env["PATH"] = os.path.dirname(codex) + ":" + env.get("PATH", "")
cwd = sys.argv[1]; out = open(sys.argv[2], "w"); script = json.load(open(sys.argv[3]))
p = subprocess.Popen([codex, "app-server"], cwd=cwd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
nid = [0]
def send(obj):
    line = json.dumps(obj); out.write(">> " + line + "\n"); out.flush(); p.stdin.write(line + "\n"); p.stdin.flush()
def req(method, params):
    nid[0] += 1; send({"jsonrpc": "2.0", "id": nid[0], "method": method, "params": params}); return nid[0]
def read(timeout):
    r, _, _ = select.select([p.stdout], [], [], timeout)
    if not r: return None
    l = p.stdout.readline()
    if not l: return ""
    out.write("<< " + l); out.flush(); return json.loads(l)
def wait_resp(i, timeout=60):
    end = time.time() + timeout
    while time.time() < end:
        m = read(1)
        if m is None: continue
        if m == "": raise SystemExit("eof")
        if m.get("id") == i and "method" not in m: return m
        answer(m)
def answer(m):
    if "method" in m and "id" in m:
        meth = m["method"]
        if "requestApproval" in meth:
            dec = script.get("approve", ["accept"]).pop(0) if script.get("approve") else "accept"
            send({"jsonrpc": "2.0", "id": m["id"], "result": {"decision": dec}})
        else:
            send({"jsonrpc": "2.0", "id": m["id"], "error": {"code": -32601, "message": "unsupported"}})
r = wait_resp(req("initialize", {"clientInfo": {"name": "openagents-desktop", "title": "OpenAgents", "version": "0.1.0"}, "capabilities": {"experimentalApi": True}}))
send({"jsonrpc": "2.0", "method": "initialized"})
if script.get("models"): wait_resp(req("model/list", {"limit": 50, "includeHidden": False}))
r = wait_resp(req("thread/start", script["thread"]))
tid = r["result"]["thread"]["id"]
for t in script["turns"]:
    params = dict(t["params"]); params["threadId"] = tid
    i = req("turn/start", params)
    stop_after = t.get("interrupt_after")
    t0 = time.time(); turn_id = None; done = False; interrupted = False
    while not done and time.time() - t0 < 240:
        m = read(1)
        if m is None:
            continue
        if m == "": raise SystemExit("eof")
        if m.get("id") == i and "result" in m: turn_id = m["result"]["turn"]["id"]
        answer(m)
        if stop_after and not interrupted and turn_id and time.time() - t0 > stop_after:
            req("turn/interrupt", {"threadId": tid, "turnId": turn_id}); interrupted = True
        if m.get("method") in ("turn/completed", "turn/failed"): done = True
p.stdin.close(); time.sleep(0.5); p.kill()
