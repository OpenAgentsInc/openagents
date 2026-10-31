# Sprints in private projects (0087_project_sprints.sql, svc-sprints): the
# schema is checked on push, and an agent may propose but not change a
# sprint's goal. test.py runs this file after test_private.py (agent, new_key,
# ADMIN_PUB come from it).
S_SLUG = "sprint-proj"
S_BOT_KEY, S_BOT = new_key("sprint-bot.key")
S_DOC = {"v": "openagents.private-project.v1", "slug": S_SLUG, "title": "Sprints", "summary": "", "rules": [],
         "members": [{"name": "Ada", "pubkey": ADMIN_PUB}], "goals": [], "days": [], "checklists": [], "links": [],
         "sprints": [{"id": "s1", "goal": "Mail the first digest", "start": "2026-11-02", "end": "2026-11-13", "status": "active",
                      "agreed_by": ["Ada"], "review": "", "retro": ""}],
         "backlog": [{"id": "b1", "text": "Resend from the API", "status": "doing"},
                     {"id": "b2", "text": "Schedules", "status": "todo", "stretch": True, "carried_from": "2026-10-30"}],
         "wip_limit": 2,
         "priority_changes": [{"at": "2026-10-31", "what": "b1 before b2", "why": "the digest needs mail", "proposed_by": "Ada", "agreed_by": ["Ada"]}]}
s_v = 0


def s_push(keyfile, doc):
    global s_v
    s, b, _ = agent(keyfile, "POST", "push", {"base": s_v, "doc": doc}, slug=S_SLUG, xff="10.0.9.1")
    if s == 429:   # a key's 30 writes a minute, shared with the files before
        time.sleep(61)
        s, b, _ = agent(keyfile, "POST", "push", {"base": s_v, "doc": doc}, slug=S_SLUG, xff="10.0.9.1")
    if s == 200:
        s_v = b["version"]
    return s, b


def s_with(change, doc=None):
    d = copy.deepcopy(doc or S_DOC)
    change(d)
    return d


s, b = s_push(os.environ["ADMIN_KEY"], S_DOC)
check("sprints: a project with a sprint, a backlog and a change log is kept", s == 200 and s_v == 1, (s, b))
bad = [
    ("a sprint with no goal", lambda d: d["sprints"][0].update(goal="")),
    ("a sprint nobody agreed to", lambda d: d["sprints"][0].update(agreed_by=[])),
    ("a sprint agreed by a stranger", lambda d: d["sprints"][0].update(agreed_by=["Zed"])),
    ("a sprint ending before it starts", lambda d: d["sprints"][0].update(end="2026-11-01")),
    ("two active sprints", lambda d: d["sprints"].append(dict(d["sprints"][0], id="s2"))),
    ("a done item without evidence", lambda d: d["backlog"][0].update(status="done")),
    ("more doing than the WIP limit", lambda d: d.update(wip_limit=1, backlog=[dict(x, status="doing") for x in d["backlog"]])),
    ("a WIP limit of zero", lambda d: d.update(wip_limit=0)),
    ("a backlog id twice", lambda d: d["backlog"].append(dict(d["backlog"][0]))),
    ("a priority change without a why", lambda d: d["priority_changes"][0].update(why="")),
    ("a backlog that is not a list", lambda d: d.update(backlog={})),
]
for name, change in bad:
    s, b = s_push(os.environ["ADMIN_KEY"], s_with(change))
    check(f"sprints: {name} is refused on push", s == 400, (s, b))
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "agent", {"pubkey": S_BOT, "name": "Bot", "role": "agent"}, slug=S_SLUG, xff="10.0.9.1")
check("sprints: an agent is registered", s == 200, (s, b))
s, b = s_push(S_BOT_KEY, s_with(lambda d: d["sprints"][0].update(goal="Something else")))
check("sprints: an agent's push that changes a sprint goal is refused", s == 403, (s, b))
s, b = s_push(S_BOT_KEY, s_with(lambda d: d["sprints"].append({"id": "s2", "goal": "Next", "start": "2026-11-16", "end": "2026-11-27",
                                                                "status": "planned", "agreed_by": ["Ada"]})))
check("sprints: an agent can't add a sprint", s == 403, (s, b))
s, b = s_push(S_BOT_KEY, s_with(lambda d: d["backlog"].reverse()))
check("sprints: an agent can't reorder the backlog", s == 403, (s, b))
s, b = s_push(S_BOT_KEY, s_with(lambda d: d["priority_changes"].append({"at": "2026-10-31", "what": "x", "why": "y", "proposed_by": "Bot", "agreed_by": ["Ada"]})))
check("sprints: an agent can't record a change as agreed", s == 403, (s, b))
s, b = s_push(S_BOT_KEY, s_with(lambda d: d.update(wip_limit=5)))
check("sprints: an agent can't raise the WIP limit", s == 403, (s, b))
s, b = s_push(S_BOT_KEY, s_with(lambda d: (d["backlog"][0].update(status="done", evidence="commit d4cdfc2e8d"),
                                            d["backlog"].append({"id": "b3", "text": "A PDF writer", "status": "todo", "proposed_by": "Bot"}),
                                            d["priority_changes"].append({"at": "2026-10-31", "what": "b3 next", "why": "digests print", "proposed_by": "Bot"}))))
check("sprints: an agent finishes an item with evidence and proposes an item and a change", s == 200 and s_v == 2, (s, b))
s, st, _ = agent(os.environ["ADMIN_KEY"], "GET", "state", slug=S_SLUG, xff="10.0.9.1")
check("sprints: the document keeps them", s == 200 and [x["id"] for x in st["doc"]["backlog"]] == ["b1", "b2", "b3"]
      and st["doc"]["sprints"][0]["goal"] == "Mail the first digest" and len(st["doc"]["priority_changes"]) == 2, (s, st))
s, b = s_push(os.environ["ADMIN_KEY"], s_with(lambda d: (d["sprints"][0].update(goal="Mail and print the digest"),
                                                         d["priority_changes"].append({"at": "2026-11-01", "what": "goal widened", "why": "print too",
                                                                                       "proposed_by": "Ada", "agreed_by": ["Ada"]})), st["doc"]))
check("sprints: a member changes the goal and logs it", s == 200 and s_v == 3, (s, b))
