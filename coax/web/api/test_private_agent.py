# A project's agent (docs/project-agent.md, migrations/0013_project_agent.sql):
# only approved projects have one, members talk to it, the runner key takes
# turns and edits inside them, and UNDO TURN puts the document back.
# test.py runs this file after test_private.py; its helpers come from there.
# test.sh approves only the fixture project "agent-proj" (OA_AGENT_PROJECTS)
# and points the wake at the fake runner below.
import http.server
import threading

wakes = []


class FakeRunner(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        wakes.append({"auth": self.headers.get("Authorization"), "body": json.loads(self.rfile.read(int(self.headers["Content-Length"])))})
        self.send_response(202)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, *a):
        pass


runner_http = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["WAKE_PORT"])), FakeRunner)
threading.Thread(target=runner_http.serve_forever, daemon=True).start()

ASLUG = "agent-proj"
CY_KEY, CY = new_key("cy.key")
RUN_KEY, RUN = new_key("runner.key")
ADOC = {
    "v": "openagents.private-project.v1", "slug": ASLUG, "title": "Agent Project", "summary": "The agent's fixture.",
    "members": [{"name": "Ada", "pubkey": ADMIN_PUB}, {"name": "Cy", "pubkey": CY}],
    "goals": [{"id": "g1", "text": "Ship the thing", "status": "agreed", "agreed_by": ["Ada", "Cy"]}],
    "days": [{"date": "2026-10-29", "milestones": [{"id": "m1", "text": "Start the deck", "owner": "Ada", "status": "todo"}]}],
    "checklists": [],
}
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": 0, "doc": ADOC}, slug=ASLUG, xff="10.1.0.1")
check("the agent's fixture project is pushed", s == 200, (s, b))
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "agent", {"pubkey": RUN, "name": "Project agent", "role": "runner"}, slug=ASLUG, xff="10.1.0.1")
check("a member registers the runner's key", s == 200, (s, b))
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "agent", {"pubkey": RUN, "name": "Project agent", "role": "runner"}, xff="10.1.0.1")
check("the same key is a runner on the unapproved project too", s == 200, (s, b))


def asession(pub, xff):
    s, inv, _ = agent(os.environ["ADMIN_KEY"], "POST", "invite", {"pubkey": pub}, slug=ASLUG, xff=xff)
    s, b, h = page("POST", f"/projects/{ASLUG}/api/redeem", {"token": inv["token"]}, xff=xff)
    return h.get("Set-Cookie", "").split("oa_project=", 1)[1].split(";", 1)[0]


ada = asession(ADMIN_PUB, "10.1.0.2")
cy = asession(CY, "10.1.0.3")


def run(op, body=None, method="POST", slug=ASLUG):
    return agent(RUN_KEY, method, op, body, slug=slug, xff="10.1.0.9")


def talk(cookie):
    s, b, _ = page("GET", f"/projects/{ASLUG}/api/talk", cookie=cookie, xff="10.1.0.4")
    return s, b.decode()


# ---- an unapproved project's agent routes are the same not found
for op, method, body in (("talk", "GET", None), ("ask", "POST", {"text": "hi"}), ("stop", "POST", {"turn": 1}),
                         ("undo", "POST", {"turn": 1}), ("agree", "POST", {"goal": "g1"})):
    s, b, _ = page(method, f"/projects/{SLUG}/api/{op}", body, cookie=session, xff="10.1.0.5")
    s2, b2, _ = page(method, f"/projects/no-such/api/{op}", body, cookie=session, xff="10.1.0.5")
    check(f"an unapproved project's {op} is the same 404 as no project", s == 404 and b == b2 == b'{"error": "not found"}', (s, b))
for op in ("next", "event", "end"):
    s, b, _ = run(op, {"turn": 1}, slug=SLUG)
    check(f"the runner's {op} on an unapproved project is not found", s == 404 and b == {"error": "not found"}, (s, b))
s, b, _ = agent(OTHER, "GET", "talk", slug=ASLUG, xff="10.1.0.6")
check("a stranger's talk is not found", s == 404 and b == {"error": "not found"})

# ---- a member asks; the runner is woken; both members see the turn
s, b = talk(ada)
check("the panel's lines start with the day's spend and the cap", s == 200 and b.startswith("Z|0.00|10.00|0|2\n"), b[:80])
n0 = len(wakes)
s, b, _ = page("POST", f"/projects/{ASLUG}/api/ask", {"text": "Mark the deck as doing"}, cookie=ada, xff="10.1.0.2")
turn = json.loads(b).get("turn")
check("a member asks the agent", s == 200 and turn, (s, b))
check("asking wakes the runner with the slug only, and a token", len(wakes) == n0 + 1 and wakes[-1]["body"] == {"slug": ASLUG}
      and wakes[-1]["auth"] == "Bearer test-wake", wakes[-1:])
s, b = talk(cy)
check("the other member sees the queued turn", f"\nU|{turn}|ask|queued|" in b and "|Ada|Mark the deck as doing|" in b and b.startswith("Z|0.00|10.00|1"), b)
s, b, _ = page("POST", f"/projects/{ASLUG}/api/ask", {"text": "x"}, cookie=ada, origin="https://evil.example", xff="10.1.0.2")
check("asking from another site is refused", s == 403)
s, b, _ = agent(BOT_KEY, "POST", "ask", {"text": "x"}, slug=ASLUG, xff="10.1.0.7")
check("someone who isn't in the project can't ask", s == 404)

# ---- the runner takes the turn and edits only inside it
s, b, _ = run("status", {"id": "m1", "status": "doing", "turn": turn})
check("the runner can't edit before the turn is running", s == 403, (s, b))
s, nx, _ = run("next", {})
check("the runner takes the turn, with the member's message", s == 200 and nx["turn"]["id"] == turn and nx["turn"]["text"] == "Mark the deck as doing"
      and nx["turn"]["by"] == "Ada" and nx["cap"] == 10, nx)
v0 = nx["version"]
s, b, _ = run("next", {})
check("one turn at a time per project", s == 200 and b["turn"] is None and b.get("busy"), b)
s, b, _ = run("status", {"id": "m1", "status": "doing", "note": "started", "by": "Project agent"})
check("the runner's write without a turn is refused", s == 403, (s, b))
s, b, _ = run("status", {"id": "m1", "status": "doing", "turn": turn, "by": "Project agent"})
check("the runner sets a milestone's status inside the turn", s == 200 and b["version"] == v0 + 1, (s, b))
s, st, _ = run("state", method="GET")
d = copy.deepcopy(st["doc"])
d["days"][0]["milestones"].append({"id": "m2", "text": "Send it to the team", "owner": "Cy", "status": "todo"})
d["goals"].append({"id": "g2", "text": "A second goal", "status": "proposed"})
s, b, _ = run("push", {"base": st["version"], "doc": d, "turn": turn})
check("the runner adds a milestone and proposes a goal", s == 200 and b["version"] == v0 + 2, (s, b))
for name, change in (("an agreed goal", lambda x: x["goals"][0].update(text="Something else")),
                     ("the members", lambda x: x["members"].append({"name": "Eve", "pubkey": "e" * 64})),
                     ("the title", lambda x: x.update(title="Renamed")),
                     ("a goal to agreed", lambda x: x["goals"][1].update(status="agreed", agreed_by=["Ada", "Cy"]))):
    bad = copy.deepcopy(d)
    change(bad)
    s, b, _ = run("push", {"base": v0 + 2, "doc": bad, "turn": turn})
    check(f"the runner can't change {name}", s == 403, (s, b))
s, b, _ = run("push", {"base": v0 + 1, "doc": d, "turn": turn})
check("a stale write is refused and the agent reads again", s == 409 and b["version"] == v0 + 2)
s, b, _ = run("event", {"turn": turn, "events": [{"type": "working", "data": {"text": "editing day 10-29"}},
                                                 {"type": "agent.tool_use", "data": {"name": "Edit", "detail": "project/plan.md"}}]})
check("the runner streams events and hears it isn't stopped", s == 200 and b == {"stop": False}, (s, b))
s, b = talk(cy)
check("the working line shows while it works", "|editing day 10-29|" in b, b)
check("each edit is a diff line under the turn", f"\nE|{turn}|" in b and "day 10-29: Start the deck → doing" in b and "day 10-29: + Send it to the team" in b
      and "goal proposed: A second goal" in b, b)
s, b, _ = run("event", {"turn": turn, "events": [{"type": "bash", "data": {}}]})
check("an unknown event type is refused", s == 400)
s, b, _ = run("end", {"turn": turn, "status": "done", "answer": "Done: the deck is doing, and I added a step.", "commit": "abc1234def",
                      "commit_message": "agent turn", "files": ["project/plan.md"], "cost_usd": 0.42})
check("the runner ends the turn with its answer, commit and cost", s == 200 and b["ok"], (s, b))
s, b = talk(ada)
check("the turn shows done, its commit and its spend, and can be undone",
      f"\nU|{turn}|ask|done|" in b and "|Done: the deck is doing, and I added a step.||abc1234|1|1|" in b and b.startswith("Z|0.42|10.00|0"), b)
check("the commit is a card with its files", "|agent.commit|abc1234 · agent turn · project/plan.md" in b, b)
s, b, _ = run("status", {"id": "m1", "status": "done", "turn": turn})
check("after the turn ends the runner can't write", s == 403)
s, tj, _ = agent(os.environ["ADMIN_KEY"], "GET", "talk", slug=ASLUG, xff="10.1.0.1")
check("members' tools get the conversation as JSON, each edit with the message that caused it",
      s == 200 and tj["turns"][-1]["text"] == "Mark the deck as doing" and any(e["type"] == "agent.edit" for e in tj["turns"][-1]["events"]), tj)

# ---- goals: proposed until every member agrees
s, b, _ = page("POST", f"/projects/{ASLUG}/api/agree", {"goal": "g2"}, cookie=ada, xff="10.1.0.2")
check("one member agreeing leaves the goal proposed", s == 200 and b == b'{"agreed": false}', (s, b))
s, b = talk(cy)
check("the panel says who agreed to a proposed goal", "\nG|g2|Ada\n" in b, b)
s, b, _ = page("POST", f"/projects/{ASLUG}/api/agree", {"goal": "g1"}, cookie=ada, xff="10.1.0.2")
check("only a proposed goal is agreed to", s == 404)

# ---- UNDO TURN
s, st, _ = run("state", method="GET")
check("before the undo the document has the turn's edits", st["version"] == v0 + 2 and len(st["doc"]["days"][0]["milestones"]) == 2)
n0 = len(wakes)
s, b, _ = page("POST", f"/projects/{ASLUG}/api/undo", {"turn": turn}, cookie=cy, xff="10.1.0.3")
undo = json.loads(b)
check("UNDO TURN is a new turn, and wakes the runner for the revert", s == 200 and undo["turn"] > turn and undo["version"] == v0 + 3 and len(wakes) == n0 + 1, (s, b))
s, st, _ = run("state", method="GET")
s2, before, _ = agent(os.environ["ADMIN_KEY"], "GET", "state", slug=ASLUG, xff="10.1.0.1")
check("UNDO TURN restores the document version from before the turn",
      st["version"] == v0 + 3 and st["doc"] == json.loads(sql(f"select doc from projects.history where slug = '{ASLUG}' and version = {v0}")), st["doc"])
s, b, _ = page("POST", f"/projects/{ASLUG}/api/undo", {"turn": turn}, cookie=cy, xff="10.1.0.3")
check("a turn is undone once", s == 409)
s, nx, _ = run("next", {})
check("the runner gets the undo with the commit to revert", nx["turn"]["kind"] == "undo" and nx["turn"]["revert"] == "abc1234def", nx)
s, b, _ = run("end", {"turn": undo["turn"], "status": "done", "commit": "fed4321", "commit_message": "revert agent turn", "files": ["project/plan.md"]})
s, b = talk(ada)
check("the undo shows as its own turn with its revert commit", f"\nU|{undo['turn']}|undo|done|" in b and f"|{undo['turn']}|" in b and "revert agent turn" in b, b)
check("the undone turn can't be undone again", f"|abc1234|1|0|{undo['turn']}|" in b, b)

# ---- the cap, STOP, and a member's edit in between
sql(f"update projects.turns set cost_usd = 10 where id = {turn}")
s, b, _ = page("POST", f"/projects/{ASLUG}/api/ask", {"text": "One more thing"}, cookie=ada, xff="10.1.0.2")
t2 = json.loads(b)["turn"]
s, b, _ = run("next", {})
check("over the cap the turn is refused", s == 200 and b["turn"] is None and b["refused"] == t2, b)
s, b = talk(ada)
check("and the page says why", f"\nU|{t2}|ask|refused|" in b and "limit for this project's agent is spent" in b, b)
sql(f"update projects.turns set cost_usd = 0 where id = {turn}")
s, b, _ = page("POST", f"/projects/{ASLUG}/api/ask", {"text": "Stop me"}, cookie=ada, xff="10.1.0.2")
t3 = json.loads(b)["turn"]
s, b, _ = run("next", {})
s, b, _ = page("POST", f"/projects/{ASLUG}/api/stop", {"turn": t3}, cookie=cy, xff="10.1.0.3")
check("a member stops a running turn", s == 200)
s, b, _ = run("event", {"turn": t3, "events": [{"type": "working", "data": {"text": "reading"}}]})
check("the runner hears STOP on its next event", s == 200 and b == {"stop": True}, b)
s, b, _ = run("end", {"turn": t3, "status": "stopped"})
s, b = talk(cy)
check("the stopped turn says who stopped it", f"\nU|{t3}|ask|stopped|" in b and "stopped by Cy" in b, b)

# ---- nothing private in the log
log = open(os.environ["API_LOG"], "rb").read()
for secret in ("Mark the deck as doing", "Send it to the team", "Done: the deck", ada, cy):
    check(f"the log never holds {secret[:8]}...", secret.encode() not in log)
