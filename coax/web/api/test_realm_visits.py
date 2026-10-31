# /realms' numbers (analytics.c realm_visits): GET /api/analytics/realms counts each realm's
# visitors, visits and minutes from page-views, leaving out automation and our team the way
# /admin/analytics does. test.py runs this file.
vev = lambda name, ct, p=None: {"ev": name, "page": "realm", "ct": ct, "p": p or {}}
vsid = lambda n: "t0000000000009" + n


def vpost(sid, events, headers=None):
    h = {"Content-Type": "text/plain", "User-Agent": "Mozilla/5.0 (Macintosh) Chrome/152.0.0.0 Safari/537.36"}
    h.update(headers or {})
    return call("POST", "/api/analytics/events", json.dumps({"sid": sid, "events": events}).encode(), h)


# two visitors in the Haunted House (one by its old address), one of them twice; one in Everglade
vpost(vsid("01"), [vev("session-start", 1, {"pid": "vis-aaaa0001"}), vev("page-view", 2, {"pid": "vis-aaaa0001", "path": "/realms/vt-house"})])
vpost(vsid("02"), [vev("session-start", 1, {"pid": "vis-aaaa0002"}), vev("page-view", 2, {"pid": "vis-aaaa0002", "path": "/realm/vt-house"}),
                   vev("page-view", 3, {"pid": "vis-aaaa0002", "path": "/realms/vt-glade/hotbar"})])
vpost(vsid("03"), [vev("session-start", 1, {"pid": "vis-aaaa0001"}), vev("page-view", 2, {"pid": "vis-aaaa0001", "path": "/realms/vt-house"})])
# our team and automation in the Haunted House count for nothing; neither does the list page itself
vpost(vsid("04"), [vev("session-start", 1, {"pid": "vis-team0004"}), vev("page-view", 2, {"pid": "vis-team0004", "path": "/realms/vt-house"})], {"Cookie": "oa_team=1"})
vpost(vsid("05"), [vev("session-start", 1, {"pid": "vis-auto0005"}), vev("page-view", 2, {"pid": "vis-auto0005", "path": "/realms/vt-house"})], {"X-Verse-Automation": "release-smoke"})
vpost(vsid("06"), [vev("session-start", 1, {"pid": "vis-agent006", "agent": "claude"}), vev("page-view", 2, {"pid": "vis-agent006", "path": "/realms/vt-house"})])
# a crawler's own name (ShapBot, 2026-10-31) is a robot, not a visitor
vpost(vsid("08"), [vev("session-start", 1, {"pid": "vis-bot00008"}), vev("page-view", 2, {"pid": "vis-bot00008", "path": "/realms/vt-house"})],
      {"User-Agent": "Mozilla/5.0 (compatible; ShapBot/0.1.0)"})
vpost(vsid("07"), [vev("session-start", 1, {"pid": "vis-aaaa0003"}), vev("page-view", 2, {"pid": "vis-aaaa0003", "path": "/realms"})])
status, body, h = call("GET", "/api/analytics/realms")
lines = {l.split("|")[0]: l.split("|")[1:] for l in body.decode().splitlines()}
check("GET /api/analytics/realms answers text", status == 200 and h.get("Content-Type", "").startswith("text/plain"), (status, h))
check("the Haunted House: two visitors, three visits (its old address counts), no team or automation",
      lines.get("vt-house", [None])[:2] == ["2", "3"], body)
check("a robot's batch is marked auto = 'bot' at the source",
      sql("select string_agg(distinct coalesce(auto, '-'), ',') from analytics.events where sid = '" + vsid("08") + "'") == "bot")
check("Everglade: one visitor, one visit", lines.get("vt-glade", [None])[:2] == ["1", "1"], body)
check("the list page and a realm nobody visited are not rows", "realms" not in lines and "" not in lines and "vt-none" not in lines, body)
check("every row's minutes are a number", all(v[2].isdigit() for v in lines.values()), body)
check("only GET", call("POST", "/api/analytics/realms", b"{}")[0] == 405)
