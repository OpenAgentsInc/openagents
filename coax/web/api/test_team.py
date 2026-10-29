# Our own testing is not usage (migrations/0037_analytics_team.sql): the team's sessions are kept with
# auto = 'team', at the source and by the backfill, and the dashboard leaves them out. test.py runs this file.
# test.sh's OA_TEAM: player p-7ea0000000000001, player kiki-7ea0000000000002, the hash of team@example.com.
import hashlib

check("the team is loaded on start: the admin's key, two players and an email's hash, not the bad value",
      sql("select string_agg(kind, ',' order by kind, value) from analytics.team") == "email_sha256,npub,player,player"
      and sql("select value from analytics.team where kind = 'email_sha256'") == hashlib.sha256(b"team@example.com").hexdigest())
check("no email is kept in the team", "@" not in sql("select string_agg(value, ' ') from analytics.team"))

tev = lambda name, ct, p=None: {"ev": name, "page": "landing", "ct": ct, "p": p or {}}
tsid = lambda n: "t0000000000000" + n


def tpost(sid, events, headers=None):
    h = {"Content-Type": "text/plain", "User-Agent": "Mozilla/5.0 (Macintosh) Chrome/152.0.0.0 Safari/537.36"}
    h.update(headers or {})
    return call("POST", "/api/analytics/events", json.dumps({"sid": sid, "events": events}).encode(), h)


autos = lambda sid: sql(f"select coalesce(string_agg(coalesce(auto, '-'), ',' order by id), '') from analytics.events where sid = '{sid}'")

# (1) the browser an admin logs in from
def login_cookies(key):
    auth = subprocess.run([os.environ["SIGN"], "--key", key, "GET", API + "/api/admin/login"], capture_output=True, text=True, check=True).stdout.strip()
    url = API + "/api/admin/login?auth=" + urllib.parse.quote(auth, safe="")
    try:
        with urllib.request.urlopen(urllib.request.Request(url)) as r:
            return r.status, r.headers.get_all("Set-Cookie") or []
    except urllib.error.HTTPError as e:
        return e.code, e.headers.get_all("Set-Cookie") or []


status, cookies = login_cookies(admin)
check("an admin's login marks the browser team: oa_team for the whole site, two years",
      status == 303 and any(c.startswith("oa_team=1; Path=/; Max-Age=63072000;") for c in cookies), cookies)
t_cookie = [c for c in cookies if c.startswith("oa_admin=")][0].split(";")[0]
# a fresh visit from a browser with no marks, then the same device with the cookie
status, body, h = tpost(tsid("01"), [tev("session-start", 1, {"pid": "dev-team-aaa1"}), tev("click", 2)])
check("a stranger is a visitor", status == 200 and autos(tsid("01")) == "-,-" and "oa_team" not in str(h), (body, h))
status, body, h = tpost(tsid("02"), [tev("session-start", 1, {"pid": "dev-team-bbb2"}), tev("click", 2)], {"Cookie": "x=y; oa_team=1"})
check("the team's browser (oa_team) is kept as auto = 'team'", status == 200 and autos(tsid("02")) == "team,team", autos(tsid("02")))
check("its session is team", sql(f"select team from analytics.sessions where sid = '{tsid('02')}'") == "t")
check("and its device is the team's", sql("select sid from analytics.team_devices where pid = 'dev-team-bbb2'") == tsid("02"))
status, body, h = tpost(tsid("03"), [tev("page-view", 1, {"pid": "dev-team-bbb2"})])
check("(3) that device later, logged out and without the cookie, is team, and gets the cookie back",
      autos(tsid("03")) == "team" and "oa_team=1" in str(h.get("Set-Cookie", "")), (autos(tsid("03")), h))
status, body, h = tpost(tsid("03"), [tev("click", 2)])
check("a later batch of a team session (no pid in it) is team too", autos(tsid("03")) == "team,team")
check("automation keeps its own mark, even from the team's browser",
      tpost(tsid("04"), [tev("click", 1)], {"Cookie": "oa_team=1", "X-Verse-Automation": "release-smoke"})[0] == 200
      and autos(tsid("04")) == "release-smoke")

# an admin's key: a site session (/login's oa_session) of an admin
admin_pub = sql("select value from analytics.team where kind = 'npub'")
site_token = "5" * 64
sql(f"insert into signin.sessions (token_sha256, pubkey, created_ms, expires_ms) values "
    f"(encode(sha256(convert_to('{site_token}', 'UTF8')), 'hex'), '{admin_pub}', 0, 99999999999999)")
tpost(tsid("05"), [tev("session-start", 1, {"pid": "dev-team-ccc3"})], {"Cookie": "oa_session=" + site_token})
check("a browser logged in (/login) as an admin is team", autos(tsid("05")) == "team")

# (2) a team player's session, and (3) the same device's earlier visits, backfilled
tpost(tsid("06"), [tev("session-start", 1, {"pid": "dev-team-ddd4"}), tev("click", 2)])
check("before anything ties it to the team, a visitor", autos(tsid("06")) == "-,-")
sql("insert into analytics.playtesters (id, at, email, player, sid, how, space) values "
    f"('tm1', extract(epoch from now()), 'Team@Example.com', 'p-somebody', '{tsid('07')}', 'email', 'web')")
tpost(tsid("07"), [tev("page-view", 1, {"pid": "dev-team-ddd4"})])
check("a join with a team email marks its session and the device", autos(tsid("07")) == "team"
      and sql("select auto from analytics.playtesters where id = 'tm1'") == "team", autos(tsid("07")))
check("and that device's earlier session is backfilled", autos(tsid("06")) == "team,team", autos(tsid("06")))

# the old playtest's import: kiki-<player>-<8 hex> sessions, its pid the player's hex
sql("insert into analytics.events (ms, sid, ev, page, p, space, import_key) values "
    "(extract(epoch from now()) * 1000, 'kiki-7ea0000000000002-0000aaaa', 'session-start', 'landing', '{\"pid\": \"7ea0000000000002\"}', 'kiki', 'tk1'), "
    "(extract(epoch from now()) * 1000, 'kiki-7ea0000000000002-0000aaaa', 'click', 'landing', '{}', 'kiki', 'tk2'), "
    "(extract(epoch from now()) * 1000, 'kiki-0123456789abcdef-0000bbbb', 'session-start', 'landing', '{\"pid\": \"0123456789abcdef\"}', 'kiki', 'tk3')")
check("an imported team player's sessions are team as they come in, a stranger's are not",
      autos("kiki-7ea0000000000002-0000aaaa") == "team,team" and autos("kiki-0123456789abcdef-0000bbbb") == "-")

# the backfill: rows kept before the team was known
sql("insert into analytics.events (ms, sid, ev, page, p, space, import_key) values "
    "(extract(epoch from now()) * 1000, 'kiki-7ea0000000000009-0000cccc', 'session-start', 'landing', '{\"pid\": \"7ea0000000000009\"}', 'kiki', 'tk4'), "
    "(extract(epoch from now()) * 1000, 'kiki-7ea0000000000009-0000dddd', 'session-start', 'landing', '{\"pid\": \"7ea0000000000009\"}', 'kiki', 'tk5')")
check("a player not yet on the team: a visitor", autos("kiki-7ea0000000000009-0000cccc") == "-")
cfg = json.dumps({"npub": [admin_pub], "player": ["p-7ea0000000000001", "kiki-7ea0000000000002", "kiki-7ea0000000000009"],
                  "email_sha256": [hashlib.sha256(b"team@example.com").hexdigest()]})
sql(f"select analytics.team_set('{cfg}')")
done = json.loads(sql("select analytics.team_tag()"))
check("adding them to the team and running the backfill marks both their sessions", done["sessions"] >= 0 and done["events"] == 2
      and autos("kiki-7ea0000000000009-0000cccc") == "team" and autos("kiki-7ea0000000000009-0000dddd") == "team", done)
check("the backfill runs twice harmlessly", json.loads(sql("select analytics.team_tag()"))["events"] == 0)
check("a stranger is never swept in", autos(tsid("01")) == "-,-" and autos("kiki-0123456789abcdef-0000bbbb") == "-")

# the dashboard: visitors without the team by default, and how many team sessions were left out
base = "/api/admin/analytics?view=overview&days=0&space=all&auto=0"
plain = call("GET", base, headers={"Cookie": t_cookie})[1].decode()
left = int(plain.split("Our team, left out|")[1].split("\n")[0])
check("the summary says how many of our team's sessions were left out", left >= 8, plain[:900])
with_team = call("GET", base + "&team=1", headers={"Cookie": t_cookie})[1].decode()
n_plain = int(plain.split("Sessions|")[1].split("\n")[0]); n_team = int(with_team.split("Sessions|")[1].split("\n")[0])
check("team=1 counts them", "Our team, counted|" in with_team and n_team == n_plain + left, (n_plain, n_team, left))
check("a team playtester is left out of the playtesters", "Playtesters|" in plain and
      int(with_team.split("Playtesters|")[1].split("\n")[0]) == int(plain.split("Playtesters|")[1].split("\n")[0]) + 1)
lst = call("GET", "/api/admin/analytics?view=sessions&days=0&auto=0", headers={"Cookie": t_cookie})[1].decode()
check("the sessions list leaves them out", tsid("02") not in lst and tsid("01") in lst)
lst = call("GET", "/api/admin/analytics?view=sessions&days=0&auto=0&team=1", headers={"Cookie": t_cookie})[1].decode()
check("and shows them, marked, with team=1", any(l.startswith(tsid("02")) and l.endswith("|our team") for l in lst.splitlines()), lst[:400])
sql("delete from analytics.playtesters where id = 'tm1'")
sql(f"delete from signin.sessions where token_sha256 = encode(sha256(convert_to('{site_token}', 'UTF8')), 'hex')")
