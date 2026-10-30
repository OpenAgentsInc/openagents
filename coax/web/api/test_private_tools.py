# The project agent's document tools (migration 0018): milestone_edit,
# goal_propose and patch (document_patch), each one new document version from
# the version the agent read, recorded under the turn, refused outside a
# running turn, on an unapproved project or to anyone but the project's
# runner or agent, and put back by UNDO TURN. test.py runs this file after
# test_private_agent.py, whose fixture project, sessions and runner key it uses.

s, b, _ = page("POST", f"/projects/{ASLUG}/api/ask", {"text": "Use every document tool"}, cookie=ada, xff="10.1.0.2")
tt = json.loads(b)["turn"]
# Wait for the turn itself to come up (not a number of tries): a loaded machine may take seconds.
deadline = time.monotonic() + 30
while True:
    s, nx, _ = run("next", {})
    if nx.get("turn") and nx["turn"]["id"] == tt:
        break
    if nx.get("turn"):   # a turn left by an earlier file: end it, and ask again at once
        run("end", {"turn": nx["turn"]["id"], "status": "done"})
        continue
    if time.monotonic() > deadline:
        break
    time.sleep(0.05)
check("the tools' turn runs", nx.get("turn", {}).get("id") == tt, nx)
s, st, _ = run("state", method="GET")
v = st["version"]
before = copy.deepcopy(st["doc"])

s, b, _ = run("milestone_edit", {"base": v, "action": "add", "date": "2026-10-30", "text": "Rehearse the pitch", "owner": "Cy", "turn": tt})
check("milestone_edit adds a milestone on a new day, with an id", s == 200 and b["version"] == v + 1 and b["id"].startswith("m-"), (s, b))
new_m = b.get("id", "")
s, st, _ = run("state", method="GET")
days = [d["date"] for d in st["doc"]["days"]]
check("the new day is in date order", days == sorted(days) and "2026-10-30" in days, days)
s, b, _ = run("milestone_edit", {"base": v + 1, "action": "reword", "id": new_m, "text": "Rehearse the pitch twice", "turn": tt})
check("milestone_edit rewords it", s == 200 and b["version"] == v + 2, (s, b))
s, b, _ = run("milestone_edit", {"base": v + 2, "action": "carry", "id": new_m, "date": "2026-10-31", "turn": tt})
s, st, _ = run("state", method="GET")
moved = [m for d in st["doc"]["days"] if d["date"] == "2026-10-31" for m in d["milestones"] if m["id"] == new_m]
check("milestone_edit carries it over, noting where it came from", s == 200 and moved and "carried over from 2026-10-30" in moved[0].get("note", ""), (s, b, st["doc"]["days"]))
s, b, _ = run("milestone_edit", {"base": v + 3, "action": "move", "id": "nope", "date": "2026-10-31", "turn": tt})
check("milestone_edit on an unknown id says so", s == 400 and "no milestone" in b["error"], (s, b))
s, b, _ = run("milestone_edit", {"base": v + 1, "action": "reword", "id": new_m, "text": "x", "turn": tt})
check("a stale milestone_edit is refused with the current version", s == 409 and b["version"] == v + 3, (s, b))

s, b, _ = run("goal_propose", {"base": v + 3, "text": "Win the demo day", "turn": tt})
check("goal_propose adds a proposed goal", s == 200 and b["id"].startswith("g-"), (s, b))
s, b, _ = run("goal_propose", {"base": v + 4, "id": "g1", "text": "Ship the thing by Friday", "turn": tt})
s, st, _ = run("state", method="GET")
g1 = [g for g in st["doc"]["goals"] if g["id"] == "g1"][0]
change = [g for g in st["doc"]["goals"] if g.get("replaces") == "g1"]
check("a change to an agreed goal is a proposal naming it; the agreed goal stays", s == 200 and g1["text"] == before["goals"][0]["text"]
      and g1["status"] == "agreed" and change and change[0]["status"] == "proposed", st["doc"]["goals"])

s, b, _ = run("patch", {"base": v + 5, "ops": [{"op": "test", "path": "/days/0/milestones/0/id", "value": "m1"},
                                               {"op": "add", "path": "/days/0/milestones/0/note", "value": "patched"},
                                               {"op": "add", "path": "/checklists/-", "value": {"id": "c1", "title": "Backlog", "items": []}}], "turn": tt})
check("document_patch applies test, add and append", s == 200 and b["version"] == v + 6, (s, b))
s, b, _ = run("milestone_edit", {"base": v + 6, "action": "add", "checklist": "c1", "text": "Book the room", "turn": tt})
check("milestone_edit adds a backlog item to a checklist", s == 200 and b["id"].startswith("i-"), (s, b))
s, b, _ = run("patch", {"base": v + 7, "ops": [{"op": "replace", "path": "/title", "value": "Renamed"}], "turn": tt})
check("document_patch can't change the title", s == 403, (s, b))
s, b, _ = run("patch", {"base": v + 7, "ops": [{"op": "replace", "path": "/goals/0/text", "value": "Other"}], "turn": tt})
check("document_patch can't change an agreed goal", s == 403, (s, b))
s, b, _ = run("patch", {"base": v + 7, "ops": [{"op": "test", "path": "/days/0/milestones/0/id", "value": "zzz"}], "turn": tt})
check("a failed test op changes nothing", s == 400 and "test failed" in b["error"], (s, b))
s, b, _ = run("patch", {"base": v + 7, "ops": [{"op": "replace", "path": "/days/0/milestones/0/status", "value": "maybe"}], "turn": tt})
check("a patch that breaks the document is refused", s == 400, (s, b))
s, b, _ = run("patch", {"base": v + 7, "ops": [{"op": "remove", "path": "/nope/1"}], "turn": tt})
check("a patch at a missing path says so", s == 400 and "nothing at path" in b["error"], (s, b))
s, b, _ = run("patch", {"base": v + 7, "ops": [{"op": "add", "path": "/days/0/milestones/0/note", "value": "x"}], "turn": tt}, slug=SLUG)
check("the tools on an unapproved project are the same 404", s == 404, (s, b))
s, b, _ = agent(CY_KEY, "POST", "patch", {"base": v + 7, "ops": [{"op": "add", "path": "/summary", "value": "x"}]}, slug=ASLUG, xff="10.1.0.4")
check("a member's key can't use the agent's tools", s == 404, (s, b))
s, b, _ = run("status", {"id": "m1", "status": "done", "note": "evidence: the deck", "turn": tt, "by": "Project agent"})
check("milestone_status and checklist_status go through status", s == 200, (s, b))
s, b, _ = run("note", {"kind": "plan", "text": "m1: draft, then review", "turn": tt, "by": "Project agent"})
check("project_plan and project_note go through note", s == 200, (s, b))
s, b, _ = run("end", {"turn": tt, "status": "done", "answer": "Used every tool.", "cost_usd": 0.01})
s, b = talk(cy)
for line in ("day 10-30: + Rehearse the pitch", "goal proposed: Win the demo day", "plan: m1: draft, then review", "Start the deck → done"):
    check(f"the edit log has \"{line}\" under the turn", any(l.startswith(f"E|{tt}|") and line in l for l in b.split("\n")), b)
s, b, _ = run("patch", {"base": v + 8, "ops": [{"op": "add", "path": "/summary", "value": "late"}], "turn": tt})
check("after the turn ends the tools are refused", s == 403, (s, b))

s, b, _ = page("POST", f"/projects/{ASLUG}/api/undo", {"turn": tt}, cookie=ada, xff="10.1.0.2")
check("UNDO TURN takes the tools' turn back", s == 200, (s, b))
s, st, _ = run("state", method="GET")
check("and the document is as it was before the turn", st["doc"] == before, (st["doc"], before))
s, nx, _ = run("next", {})
if nx.get("turn"):
    run("end", {"turn": nx["turn"]["id"], "status": "done"})
