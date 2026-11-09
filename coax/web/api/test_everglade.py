# Everglade's report (/admin/analytics, EVERGLADE: admin.c everglade) and the team link that marks an
# admin's browser on everglade.world (admin.c team_link, analytics.c team_mark). test.py runs this file.
import urllib.parse


class EgNoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


urllib.request.install_opener(urllib.request.build_opener(EgNoRedirect))   # as test_dashboard.py leaves it


def eg_post(sid, events, headers=None, auto=None):
    b = {"sid": sid, "events": events}
    if auto:
        b["auto"] = auto
    h = {"Content-Type": "text/plain", "User-Agent": "Mozilla/5.0 (Macintosh) Chrome/152.0.0.0 Safari/537.36", "Host": "everglade.world"}
    h.update(headers or {})
    return call("POST", "/api/analytics/events", json.dumps(b).encode(), h)


def eg_admin():
    __import__("time").sleep(1.1)   # a signed login in the same second as an earlier one is a replay
    sig = subprocess.run([os.environ["SIGN"], "--key", os.environ["ADMIN_KEY"], "GET", API + "/api/admin/login"],
                         capture_output=True, text=True, check=True).stdout.strip()
    _, _, headers = call("GET", "/api/admin/login?auth=" + urllib.parse.quote(sig, safe=""))
    return headers.get("Set-Cookie", "").split(";")[0]


eg = lambda ev, ct, p=None: {"ev": ev, "page": "everglade2", "ct": ct, "p": p or {}}
# a new player through the front door, all the way to a felled wolf
new = "c0e9000000000000a1"
eg_post(new, [
    eg("session-start", 1, {"pid": "egv1", "os": "macOS", "browser": "Chrome 152", "gpu": True}),
    eg("page-view", 2, {"pid": "egv1", "path": "/play", "q": "via=portal"}),
    eg("load-engine", 3, {"ms": 1800}), eg("load-world", 4, {"ms": 4200}), eg("portal-shown", 5, {"id": "door"}),
    eg("create-open", 6, {"id": "portal"}), eg("create-pick", 7, {"id": "hair-style"}), eg("create-class", 8, {"id": "ranger"}),
    eg("create-name", 9, {"ok": True}), eg("create-done", 10, {"id": "ranger", "n": 1, "s": 40}),
    eg("run-end", 11, {"s": 23}), eg("spawn", 12, {"id": "create", "class": "ranger", "n": 1}), eg("room-join", 13, {"ms": 30000}),
    eg("tally", 14, {"cast_longbow": 6, "hit_wolf": 3, "kill_wolf": 1, "xp": 50}),
    eg("death", 15, {"id": "goblin"}),
])
# one whose browser has no WebGPU, from the host's beacon
nogpu = "c0e9000000000000b2"
eg_post(nogpu, [eg("page-view", 1, {"pid": "egv2", "path": "/play"}), eg("load-fail", 2, {"msg": "no WebGPU", "webgpu": False})])
# a returning player with a kept character
back = "c0e9000000000000c3"
eg_post(back, [eg("page-view", 1, {"pid": "egv1", "path": "/play"}), eg("load-engine", 2, {"ms": 900}), eg("load-world", 3, {"ms": 2000}),
               eg("spawn", 4, {"id": "kept", "class": "ranger", "n": 1})])
# through the portal on openagents.com/newlander: /play?via=portal, come from openagents.com
lander = "c0e9000000000000e5"
eg_post(lander, [eg("page-view", 1, {"pid": "egv3", "from": "openagents.com", "path": "/play", "q": "via=portal"}), eg("load-engine", 2, {"ms": 1500})])
# the front door on a browser without WebGPU: its own page view and PLAY NOW (realms.ts), no game
door = "c0e9000000000000f6"
eg_post(door, [eg("page-view", 1, {"pid": "egv4", "from": "news.ycombinator.com", "path": "/"}), eg("play-now-click", 2, {"id": "door", "at": "hero", "gpu": False})])
# the team's own play on everglade.world: a team-link browser
cookie = eg_admin()
status, _, headers = call("GET", "/api/admin/team-link?host=everglade.world", headers={"Cookie": cookie})
loc = headers.get("Location", "")
check("an admin's team link goes to everglade.world's own mark, once", status == 303 and loc.startswith("https://everglade.world/api/analytics/team?t="), (status, headers))
check("a team link for another host is refused", call("GET", "/api/admin/team-link?host=evil.example", headers={"Cookie": cookie})[0] == 400)
check("a team link needs an admin", call("GET", "/api/admin/team-link?host=everglade.world")[0] == 401)
t = loc.split("t=")[-1]
status, _, h2 = call("GET", "/api/analytics/team?t=" + t, headers={"Host": "openagents.com"})
check("a team link works only on the host it was made for", status == 400, status)
status, _, h2 = call("GET", "/api/analytics/team?t=" + t, headers={"Host": "everglade.world"})
check("everglade.world marks the browser as our team and goes to its front page",
      status == 303 and h2.get("Location") == "/" and "oa_team=1" in h2.get("Set-Cookie", ""), (status, h2))
check("the link is used up", call("GET", "/api/analytics/team?t=" + t, headers={"Host": "everglade.world"})[0] == 400)
check("only the link's hash is kept", sql("select count(*) from analytics.team_links where token_sha256 = '" + t + "'") == "0")
team = "c0e9000000000000d4"
eg_post(team, [eg("page-view", 1, {"pid": "egteam", "path": "/play"}), eg("spawn", 2, {"id": "kept"})], {"Cookie": "oa_team=1"})
check("the team's play on everglade.world is kept as the team's", sql(f"select string_agg(distinct auto, ',') from analytics.events where sid = '{team}'") == "team")

status, body, _ = call("GET", "/api/admin/analytics?view=everglade&days=0&space=all&auto=0", headers={"Cookie": cookie})
text = body.decode()
check("EVERGLADE: the admin sees how far visits got, step by step",
      status == 200 and "#EVERGLADE: HOW FAR VISITS GOT|w|STEP|GOT THIS FAR|OF ALL WHO OPENED IT|STOPPED HERE" in text
      and "\nOpened the game|5|100%|" in text and "\nFelled a wolf, goblin or the guard|1|20%|1" in text and "\nIn Everglade|2|40%|" in text, text[:1500])
check("EVERGLADE: the ways in", "\neverglade.world front door|2|1|0" in text and "\neverglade.world/play|2|1|1" in text
      and "\nopenagents.com/newlander portal|1|0|0" in text, text[:2500])
check("EVERGLADE: loading and what went wrong",
      "\nFailed to load|1" in text and "\nBrowser without WebGPU|1" in text and "\nFailed to load|no WebGPU|1|1|" in text, text)
check("EVERGLADE: what they did, from the tallies and the acts",
      "\nkill_wolf|1|1" in text and "\ncast_longbow|1|6" in text and "\ndeath goblin|1|1" in text, text)
check("EVERGLADE: the sessions open a timeline, and the team is left out",
      "#EVERGLADE: SESSIONS|s|" in text and "\n" + new + "|" in text and "|ranger|1|1|" in text and team not in text, text)
check("EVERGLADE: a person who came back on a later day isn't counted twice in a day", "#EVERGLADE: BY DAY|w|" in text, text)
with_team = call("GET", "/api/admin/analytics?view=everglade&days=0&space=all&auto=0&team=1", headers={"Cookie": cookie})[1].decode()
check("OUR TEAM counts the team's play too", "\n" + team + "|" in with_team, with_team[-800:])
overview = call("GET", "/api/admin/analytics?view=overview&days=0&space=all&auto=0", headers={"Cookie": cookie})[1].decode()
check("the overview has an EVERGLADE.WORLD card", "#EVERGLADE.WORLD||WHAT|SESSIONS|TIMES" in overview and "\neverglade2|" in overview, overview[-2000:])
