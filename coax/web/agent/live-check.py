#!/usr/bin/env python3
# The project agent end to end against a deployed oa-api and runner, as a
# member on the page would: sign in, ask, wait for the turn, check what the
# agent did, UNDO TURN, and check the project is back as it was. Tooling for
# the owner's checks; it never prints keys or the project's contents.
#
#   live-check.py --origin https://staging.openagents.com --slug SLUG --key MEMBER.key tools
#       a message that asks for every project tool and a file edit: each tool
#       was called, the document's version moved with the edits logged under
#       the turn, the commit was pushed; UNDO TURN puts both back
#   live-check.py ... --slug SLUG one      one milestone status and one file line, then UNDO TURN
#   --file PATH picks the repository file the check appends to (default project/plan.md)
#   live-check.py ... --slug SLUG refused  this project's agent answers 404, like no project
#   live-check.py ... --slug SLUG capped   the next ask is refused with the day's limit
#   live-check.py ... --slug SLUG key --token FILE --other OTHER.key
#       KEY: the member saves the subscription token in FILE and their turn runs
#       on it; a second member (OTHER.key, invited by the first) with none runs
#       on OpenAgents' key, never the first member's; REMOVE puts the first
#       back on ours. The token is read from FILE and never printed.
import json, os, subprocess, sys, tempfile, time, urllib.error, urllib.request

args = sys.argv[1:]
def opt(name, default=None):
    if name in args:
        i = args.index(name); v = args[i + 1]; del args[i:i + 2]; return v
    return default
ORIGIN = opt("--origin", "https://staging.openagents.com")
SLUG = opt("--slug")
FILE = opt("--file", "project/plan.md")
KEY = opt("--key", os.path.expanduser("~/work/.secrets/openagents-analytics-admin.key"))
SIGN = opt("--sign", os.path.join(os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache")), "oa-admin", "sign"))
TOKEN = opt("--token")
OTHER = opt("--other")
MODE = args[0] if args else "tools"
failed = 0


def check(name, ok, detail=""):
    global failed
    print(("  ok    " if ok else "  FAIL  ") + name + ("" if ok or not detail else f"  {detail}"[:600]))
    if not ok:
        failed += 1


def http(method, url, body=None, headers=None):
    req = urllib.request.Request(url, method=method, data=None if body is None else body, headers=headers or {})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.status, r.read().decode(), dict(r.headers)
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode(), dict(e.headers)


def signed(method, op, body=None):
    url = f"{ORIGIN}/api/private/{SLUG}/{op}"
    raw = None if body is None else json.dumps(body).encode()
    with tempfile.NamedTemporaryFile() as f:
        if raw is not None:
            f.write(raw); f.flush()
        auth = subprocess.run([SIGN, "--key", KEY, method, url] + ([f.name] if raw is not None else []),
                              capture_output=True, text=True, check=True).stdout.strip()
    h = {"Authorization": "Nostr " + auth}
    if raw is not None:
        h["Content-Type"] = "application/json"
    time.sleep(1.1)   # an identical request in the same second is the same event, good once
    s, b, _ = http(method, url, raw, h)
    try:
        return s, json.loads(b)
    except ValueError:
        return s, b


def session(key=None):
    pub = subprocess.run([SIGN, "--key", key or KEY, "--pubkey"], capture_output=True, text=True, check=True).stdout.strip()
    s, inv = signed("POST", "invite", {"pubkey": pub})
    if s != 200:
        return None
    s, b, h = http("POST", f"{ORIGIN}/projects/{SLUG}/api/redeem", json.dumps({"token": inv["token"]}).encode(),
                   {"Origin": ORIGIN, "Content-Type": "application/json"})
    c = next((v for k, v in h.items() if k.lower() == "set-cookie" and "oa_project=" in v), "")
    return c.split("oa_project=", 1)[1].split(";", 1)[0] if s == 200 and c else None


def page(op, body, cookie):
    s, b, _ = http("POST", f"{ORIGIN}/projects/{SLUG}/api/{op}", json.dumps(body).encode(),
                   {"Origin": ORIGIN, "Content-Type": "application/json", "Cookie": f"oa_project={cookie}"})
    try:
        return s, json.loads(b)
    except ValueError:
        return s, b


def turn_of(tid, until, seconds=420):
    t = {}
    for _ in range(seconds // 3):
        s, tj = signed("GET", "talk")
        t = next((x for x in tj.get("turns", []) if x["id"] == tid), {}) if s == 200 else {}
        if t.get("status") in until:
            return t, tj
        time.sleep(2)
    return t, {}


def edits(t):
    return [e for e in t.get("events", []) if e["type"] == "agent.edit"]


def commit_of(t):
    return next((e["data"] for e in t.get("events", []) if e["type"] == "agent.commit"), None)


if MODE == "refused":
    s, b = signed("GET", "talk")
    nope = SLUG; SLUG = "no-such-project-x"; s3, b3 = signed("GET", "talk"); SLUG = nope
    check("a project without the agent answers talk with 404", s == 404, (s, b))
    check("the same answer as a project that doesn't exist", s == s3 and b == b3, (b, b3))
    cookie = session()
    if cookie:
        s, b = page("ask", {"text": "hello"}, cookie)
        check("asking on it is the same 404", s == 404, (s, b))
    sys.exit(1 if failed else 0)

cookie = session()
check("a member signs in", cookie is not None)
if not cookie:
    sys.exit(1)
s, st = signed("GET", "state")
v0, doc0 = st["version"], st["doc"]

if MODE == "key":
    if OTHER and not os.path.exists(OTHER):
        subprocess.run([SIGN, "--new", OTHER], capture_output=True, check=True)
    cookie_b = session(OTHER)
    check("a second member signs in", cookie_b is not None)

    def talk(c):
        s, b, _ = http("GET", f"{ORIGIN}/projects/{SLUG}/api/talk", None, {"Cookie": f"oa_project={c}"})
        return b if s == 200 else ""

    def ask(c, text):
        s, b = page("ask", {"text": text}, c)
        tid = b.get("turn") if isinstance(b, dict) else None
        for _ in range(150):
            u = next((l.split("|") for l in talk(c).split("\n") if l.startswith(f"U|{tid}|")), None)
            if u and u[3] in ("done", "failed", "stopped", "refused"):
                return u
            time.sleep(2)
        return u

    def kline(c):
        return next((l.split("|") for l in talk(c).split("\n") if l.startswith("K|")), None)

    page("key_remove", {}, cookie); page("key_remove", {}, cookie_b)
    s, b = page("key", {"credential": open(TOKEN).read().strip()}, cookie)
    check("the member's subscription token is checked and saved; kind and last four only", s == 200 and isinstance(b, dict) and b.get("kind") == "oauth" and "sk-ant" not in json.dumps(b), (s, str(b)[:120]))
    check("their panel shows their key", (kline(cookie) or [""])[1:2] == ["oauth"], kline(cookie))
    u = ask(cookie, "Automated check: reply with exactly the word ok. Use no tools.")
    check("their turn runs on their own token", u and u[3] == "done" and u[15] == "own", u and (u[3], u[8][:200], u[15]))
    check("the other member's panel shows no key of theirs", kline(cookie_b) == ["K", "", "", "0"], kline(cookie_b))
    u = ask(cookie_b, "Automated check: reply with exactly the word ok. Use no tools.")
    check("the other member's turn runs on OpenAgents' key, never the first member's", u and u[3] == "done" and u[15] == "openagents", u and (u[3], u[8][:200], u[15]))
    s, b = page("key_remove", {}, cookie)
    check("REMOVE deletes it", s == 200 and kline(cookie) == ["K", "", "", "0"], kline(cookie))
    u = ask(cookie, "Automated check: reply with exactly the word ok. Use no tools.")
    check("with none saved, their turn runs on OpenAgents' key under the cap", u and u[3] == "done" and u[15] == "openagents", u and (u[3], u[8][:200], u[15]))
    sys.exit(1 if failed else 0)

if MODE == "capped":
    s, b = page("ask", {"text": "Automated check: say hello."}, cookie)
    tid = b.get("turn") if isinstance(b, dict) else None
    t, _ = turn_of(tid, ("refused", "done", "failed"), 60)
    check("over the day's limit the turn is refused", t.get("status") == "refused" and "limit" in (t.get("answer", "") + t.get("error", "")), t)
    s, st = signed("GET", "state")
    check("and nothing changed", st["version"] == v0)
    sys.exit(1 if failed else 0)

days = doc0.get("days", [])
ms = next((m for d in days for m in d.get("milestones", [])), None)
item = next((i for c in doc0.get("checklists", []) for i in c.get("items", [])), None)
other = "doing" if ms and ms["status"] != "doing" else "todo"
if MODE == "tools":
    text = ("This is an automated check of your tools. Do exactly these steps, in order, each once:\n"
            "1. project_get.\n"
            f"2. milestone_status: milestone {ms['id']} to {other}, evidence \"live check\".\n"
            + (f"3. checklist_status: item {item['id']} to {'done' if item['status'] != 'done' else 'todo'}, evidence \"live check\".\n" if item else "3. (no checklist item: skip)\n")
            + "4. milestone_edit: add a milestone dated 2026-12-01 with the text \"Live check milestone\".\n"
            "5. goal_propose: the goal \"Live check goal\".\n"
            "6. project_note: \"Live check note\".\n"
            f"7. project_plan: for milestone {ms['id']}, the plan \"Live check plan\".\n"
            "8. document_patch: add /summary_check with the value \"live check\" (read the version first).\n"
            f"9. Append the line \"- live check\" to {FILE} in the repository.\n"
            "Read the document again before each write that needs its version. Then say which steps worked.")
    want_tools = ["project_get", "milestone_status", "milestone_edit", "goal_propose", "project_note", "project_plan", "document_patch"] + (["checklist_status"] if item else [])
else:
    text = (f"Automated check: set milestone {ms['id']} to {other} with the evidence \"live check\", and append the line \"- live check\" "
            f"to {FILE}. Nothing else.")
    want_tools = ["milestone_status"]

s, b = page("ask", {"text": text}, cookie)
check("the member's message is queued", s == 200 and b.get("turn"), (s, b))
tid = b["turn"]
t, tj = turn_of(tid, ("done", "failed", "stopped", "refused"))
check("the turn ends done", t.get("status") == "done", {k: t.get(k) for k in ("status", "error", "answer")})
called = [e["data"].get("name") for e in t.get("events", []) if e["type"] == "agent.tool_use"]
for name in want_tools:
    check(f"the agent called {name}", name in called, called)
check(f"the agent edited {FILE}", "Edit" in called or "Write" in called, called)
s, st = signed("GET", "state")
check("the document's version moved", st["version"] > v0 and t.get("version_after") == st["version"], (v0, st["version"], t.get("version_after")))
ed = edits(t)
check("the edits are logged under the turn", len(ed) >= (4 if MODE == "tools" else 1), [e["data"].get("line") for e in ed])
c = commit_of(t)
check("the commit was pushed, named for the turn", c is not None and c.get("message", "").startswith(f"agent turn {tid}:") and FILE in c.get("files", []), c)
print(f"  ..    turn {tid}: {len(called)} tool calls, {len(ed)} edits, commit {c and c.get('sha', '')[:7]}, cost ${t.get('cost_usd')}")

s, b = page("undo", {"turn": tid}, cookie)
check("UNDO TURN is accepted", s == 200 and b.get("turn"), (s, b))
u, _ = turn_of(b.get("turn"), ("done", "failed"))
check("the undo turn ends done", u.get("status") == "done", {k: u.get(k) for k in ("status", "error")})
s, st = signed("GET", "state")
check("the document is back as it was before the turn", st["doc"] == doc0, st["version"])
uc = commit_of(u)
check("the turn's commit was reverted and pushed", uc is not None and uc.get("message", "") == f"agent turn {u.get('id')}: undo turn {tid}", uc)
print(f"  ..    undo turn {u.get('id')}: commit {uc and uc.get('sha', '')[:7]}")
sys.exit(1 if failed else 0)
