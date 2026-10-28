# Private projects (private.c, migrations/0007_private_projects.sql): who may
# read and write, that nobody else learns anything, sign-in links and
# sessions, CSRF and rate limits, and that nothing private reaches the log.
# test.py runs this file; call, check, sql, API and SIGN come from it.
import copy
import statistics
import time

import tempfile as _t

SIGN_BIN = os.environ["SIGN"]
key_dir = _t.mkdtemp()


def new_key(name):
    path = os.path.join(key_dir, name)
    pub = subprocess.run([SIGN_BIN, "--new", path], capture_output=True, text=True, check=True).stdout.strip()
    return path, pub


ADMIN_PUB = subprocess.run([SIGN_BIN, "--key", os.environ["ADMIN_KEY"], "--pubkey"], capture_output=True, text=True, check=True).stdout.strip()
OTHER = os.environ["OTHER_KEY"]
BEA_KEY, BEA = new_key("bea.key")
BOT_KEY, BOT = new_key("bot.key")
SLUG = "test-proj"
ME = {"Origin": API}


def signed(keyfile, method, path, body=None):
    args = [SIGN_BIN, "--key", keyfile, method, API + path]
    if body is not None:
        f = _t.NamedTemporaryFile(delete=False)
        f.write(body)
        f.close()
        args.append(f.name)
    return "Nostr " + subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()


last_signed = {}


def agent(keyfile, method, op, body=None, slug=SLUG, xff="10.0.0.1"):
    path = f"/api/private/{slug}/{op}"
    raw = None if body is None else json.dumps(body).encode()
    # The same request signed twice in one second is the same event, and an event is good once.
    same = (keyfile, method, path, raw)
    if time.time() - last_signed.get(same, 0) < 1.1:
        time.sleep(1.1)
    last_signed[same] = time.time()
    h = {"Authorization": signed(keyfile, method, path, raw), "X-Forwarded-For": xff}
    if raw is not None:
        h["Content-Type"] = "application/json"
    s, b, hd = call(method, path, raw, h)
    try:
        return s, json.loads(b), hd
    except ValueError:
        return s, b, hd


def page(method, path, body=None, cookie=None, origin=API, xff="10.0.0.2"):
    h = {"X-Forwarded-For": xff}
    if origin:
        h["Origin"] = origin
    if cookie:
        h["Cookie"] = "oa_project=" + cookie
    raw = None if body is None else json.dumps(body).encode()
    return call(method, path, raw, h)


DOC = {
    "v": "openagents.private-project.v1", "slug": SLUG, "title": "Test Project", "summary": "A project for the checks.",
    "members": [{"name": "Ada", "pubkey": ADMIN_PUB}, {"name": "Bea", "pubkey": BEA}],
    "rules": ["Goals change only together."],
    "goals": [{"id": "g1", "text": "Ship the thing", "status": "agreed", "agreed_by": ["Ada", "Bea"], "source": "plan.md"}],
    "days": [{"date": "2026-10-28", "milestones": [{"id": "m1", "text": "Write it", "owner": "Ada", "status": "todo"}]}],
    "checklists": [{"id": "c1", "title": "Before", "source": "x", "items": [{"id": "i1", "text": "Tests", "status": "doing", "evidence": "some"}]}],
    "links": [{"label": "Old dashboard", "url": "https://example.com/a"}],
}

# ---- nobody learns anything: no project, no session, a stranger
s_none, b_none, h_none = page("GET", "/projects/no-such")
s_locked, b_locked, h_locked = page("GET", f"/projects/{SLUG}")
check("an unknown slug's page is the locked shell (404)", s_none == 404 and b"ask a member for a sign-in link" in b_none)
check("every private answer is no-store, private", "no-store, private" in h_none.get("Cache-Control", ""))
check("private answers vary by credential, can't be framed, send no referrer",
      "Cookie" in h_none.get("Vary", "") and "Authorization" in h_none.get("Vary", "")
      and h_none.get("X-Frame-Options") == "DENY" and h_none.get("Referrer-Policy") == "no-referrer", h_none)
for path in ["/projects/no-such/api/state", f"/projects/{SLUG}/api/state", "/api/private/no-such/state", f"/api/private/{SLUG}/state"]:
    s, b, _ = page("GET", path)
    check(f"{path} without a session: 404, nothing else", s == 404 and b == b'{"error": "not found"}', (s, b))
s, b, _ = agent(OTHER, "POST", "push", {"base": 0, "doc": DOC})
check("the first push is an admin's only", s == 404, (s, b))
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": 0, "doc": dict(DOC, slug="other")})
check("a document's slug is the project's", s == 400, (s, b))
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": 0, "doc": DOC})
check("an admin's first push makes version 1", s == 200 and b == {"version": 1}, (s, b))
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": 0, "doc": DOC})
check("a push from a stale version is refused", s == 409 and b["version"] == 1, (s, b))

s_locked, b_locked, h_locked = page("GET", f"/projects/{SLUG}")
check("a project's page, signed out, is the same locked shell as no project",
      s_locked == 404 and b_locked.replace(SLUG.encode(), b"no-such") == b_none, (s_locked,))
check("the locked shell names nothing of the project", b"Test Project" not in b_locked and b"Ada" not in b_locked)
stranger = [agent(OTHER, "GET", "state", slug=sl) for sl in (SLUG, "no-such")]
check("a stranger's signature gets the same 404 for a project and for none",
      all(s == 404 and b == {"error": "not found"} for s, b, _ in stranger))
s, b, _ = page("GET", f"/projects/{SLUG}/api/state", cookie="ab" * 32)
check("a made-up cookie is nobody", s == 404 and b == b'{"error": "not found"}')


def timed(slug):
    t = time.perf_counter()
    page("GET", f"/projects/{slug}/api/state", cookie="cd" * 32)
    return time.perf_counter() - t


for _ in range(5):
    timed(SLUG), timed("no-such")
gap = abs(statistics.median([timed(SLUG) for _ in range(30)]) - statistics.median([timed("no-such") for _ in range(30)]))
check(f"a project and no project answer in the same time ({gap * 1000:.2f} ms apart)", gap < 0.003)

# ---- members read; invites make sessions
s, b, _ = agent(os.environ["ADMIN_KEY"], "GET", "state")
check("a member reads the state", s == 200 and b["doc"]["title"] == "Test Project" and b["you"] == {"name": "Ada", "role": "member"}, (s, b))
check("the state carries the live numbers", any(r["label"] == "Sessions" for r in b["analytics"]))
s, b, _ = agent(OTHER, "POST", "invite", {"pubkey": BEA})
check("a stranger can't make an invite", s == 404)
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "invite", {"pubkey": OTHER and "e" * 64})
check("invites are for members only", s == 400, (s, b))
s, inv, _ = agent(os.environ["ADMIN_KEY"], "POST", "invite", {"pubkey": BEA})
token = inv.get("token", "")
check("a member invites a member", s == 200 and len(token) == 64, (s, inv))
check("only the invite's hash is kept", sql(f"select count(*) from projects.invites where token_sha256 = encode(sha256('{token}'), 'hex')") == "1"
      and sql(f"select count(*) from projects.invites where token_sha256 = '{token}'") == "0")
s, _, _ = page("POST", f"/projects/{SLUG}/api/redeem", {"token": token}, origin=None)
check("a sign-in without an Origin is refused (CSRF)", s == 403)
s, _, _ = page("POST", f"/projects/{SLUG}/api/redeem", {"token": token}, origin="https://evil.example")
check("a sign-in from another site is refused (CSRF)", s == 403)
s, _, _ = page("POST", "/projects/no-such/api/redeem", {"token": token})
check("an invite works only for its project", s == 404)
s, b, h = page("POST", f"/projects/{SLUG}/api/redeem", {"token": token})
cookie_header = h.get("Set-Cookie", "")
check("an invite signs in once, with a 30-day cookie", s == 200 and "Max-Age=2592000" in cookie_header, (s, cookie_header))
for flag in ("HttpOnly", "Secure", "SameSite=Lax", f"Path=/projects/{SLUG};"):
    check(f"the cookie is {flag.rstrip(';')}", flag in cookie_header)
session = cookie_header.split("oa_project=", 1)[1].split(";", 1)[0]
check("the session's token isn't in the answer's body", session.encode() not in b)
check("only the session's hash is kept", sql(f"select count(*) from projects.sessions where token_sha256 = encode(sha256('{session}'), 'hex') and pubkey = '{BEA}'") == "1")
s, _, _ = page("POST", f"/projects/{SLUG}/api/redeem", {"token": token})
check("an invite is good once", s == 404)
s, inv2, _ = agent(os.environ["ADMIN_KEY"], "POST", "invite", {"pubkey": BEA})
sql(f"update projects.invites set expires_ms = 1 where token_sha256 = encode(sha256('{inv2['token']}'), 'hex')")
s, _, _ = page("POST", f"/projects/{SLUG}/api/redeem", {"token": inv2["token"]})
check("an invite expires", s == 404)
check("invites last 24 hours", sql("select bool_and(expires_ms - created_ms = 86400000) from projects.invites where expires_ms > 1") == "t")

s, b, _ = page("GET", f"/projects/{SLUG}", cookie=session)
check("a member's page is the member shell, its title generic", s == 200 and b"<title>Project" in b and b"Test Project" not in b)
s, b, _ = page("GET", f"/projects/{SLUG}/api/state", cookie=session)
text = b.decode()
check("the page's state comes as lines", s == 200 and text.startswith("Y|Bea|member\n") and "\nG|g1|agreed|Ada, Bea|Ship the thing" in text, text[:200])
check("the lines have the day, the checklist, the numbers and the links",
      "\nM|2026-10-28|m1|todo|Ada|Write it|" in text and "\nI|c1|i1|doing|Tests|some" in text and "\nN|Sessions|" in text and "\nL|Old dashboard|https://example.com/a" in text)
s, _, _ = page("GET", "/projects/no-such/api/state", cookie=session)
check("a session is for its own project only", s == 404)

# ---- messages, from the page
s, b, _ = page("POST", f"/projects/{SLUG}/api/message", {"text": "Please look at the deck"}, cookie=session, origin="https://evil.example")
check("a message from another site is refused", s == 403)
s, b, _ = page("POST", f"/projects/{SLUG}/api/message", {"text": "Please look at the deck"}, cookie=session)
mid = json.loads(b).get("id")
check("a member sends a message from the page", s == 200 and mid)

# ---- agents
s, b, _ = agent(BOT_KEY, "GET", "state")
check("an agent nobody registered is nobody", s == 404)
s, b, _ = agent(BOT_KEY, "POST", "agent", {"pubkey": BOT, "name": "Bot", "role": "agent"})
check("an agent can't register itself", s == 404)
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "agent", {"pubkey": BOT, "name": "Bot", "role": "agent"})
check("a member registers an agent", s == 200, (s, b))
s, st, _ = agent(BOT_KEY, "GET", "state")
check("the agent reads the state", s == 200 and st["you"]["role"] == "agent")
v = st["version"]
s, b, _ = agent(BOT_KEY, "GET", "inbox")
check("the agent's inbox has the message", s == 200 and [m["id"] for m in b] == [mid], b)
s, b, _ = agent(BOT_KEY, "POST", "message", {"text": "On it", "reply_to": mid, "by": "Bot 1"})
check("the agent replies", s == 200)
s, b, _ = agent(BOT_KEY, "GET", "inbox")
check("an answered message leaves the inbox", s == 200 and b == [])
s, b, _ = agent(BOT_KEY, "POST", "note", {"kind": "plan", "text": "1. read\n2. write", "by": "Bot 1"})
check("the agent posts a plan", s == 200)
s, b, _ = agent(BOT_KEY, "POST", "note", {"kind": "rumor", "text": "x"})
check("a note's kind is note, plan or status", s == 400)
s, b, _ = agent(BOT_KEY, "POST", "status", {"id": "m1", "status": "doing", "note": "started", "by": "Bot 1"})
check("the agent moves a milestone", s == 200 and b["version"] == v + 1, (s, b))
v += 1
s, b, _ = agent(BOT_KEY, "POST", "status", {"id": "nope", "status": "done"})
check("a status for an unknown id is refused", s == 404)
s, st, _ = agent(BOT_KEY, "GET", "state")
check("the status change is in the document and the activity",
      st["doc"]["days"][0]["milestones"][0]["status"] == "doing" and st["activity"][0]["kind"] == "status")
base_doc = st["doc"]


def agent_push(change):
    d = copy.deepcopy(base_doc)
    change(d)
    return agent(BOT_KEY, "POST", "push", {"base": v, "doc": d})


s, b, _ = agent_push(lambda d: d["goals"][0].update(text="Ship something else"))
check("an agent can't edit an agreed goal", s == 403, (s, b))
s, b, _ = agent_push(lambda d: d["goals"].pop(0))
check("an agent can't remove an agreed goal", s == 403)
s, b, _ = agent_push(lambda d: d["goals"].append({"id": "g2", "text": "x", "status": "agreed", "agreed_by": ["Ada", "Bea"]}))
check("an agent can't add an agreed goal", s == 403)
s, b, _ = agent_push(lambda d: d["members"].append({"name": "Eve", "pubkey": "e" * 64}))
check("an agent can't change the members", s == 403)
s, b, _ = agent_push(lambda d: d.update(title="Renamed"))
check("an agent can't change anything else", s == 403)
s, b, _ = agent(BOT_KEY, "POST", "invite", {"pubkey": BEA})
check("an agent can't make invites", s == 403)
s, b, _ = agent_push(lambda d: (d["goals"].append({"id": "g3", "text": "Try a second program", "status": "proposed", "source": "Bot 1"}),
                                d["checklists"][0]["items"][0].update(status="done", evidence="all green")))
check("an agent proposes a goal and ticks an item", s == 200 and b["version"] == v + 1, (s, b))
v += 1
for who in ([], ["Zed"]):
    s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": v, "doc": dict(DOC, goals=DOC["goals"] + [{"id": "g4", "text": "y", "status": "agreed", "agreed_by": who}])})
    check(f"an agreed goal names members who agreed ({who})", s == 400, (s, b))
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": v, "doc": dict(DOC, goals=DOC["goals"] + [{"id": "g4", "text": "y", "status": "agreed", "agreed_by": ["Ada"]}])})
check("a goal one member agreed to is agreed by that member", s == 200 and b["version"] == v + 1, (s, b))
v += 1
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": v, "doc": dict(DOC, title="Test Project 2")})
check("a member pushes anything valid", s == 200 and b["version"] == v + 1)
check("every push is kept", sql(f"select count(*) from projects.history where slug = '{SLUG}'") == str(v + 1))
sql(f"update projects.sessions set expires_ms = 1 where pubkey = '{BEA}'")
s, _, _ = page("GET", f"/projects/{SLUG}/api/state", cookie=session)
check("a session expires", s == 404)
s, b, _ = page("POST", f"/projects/{SLUG}/api/message", {"text": "late"}, cookie=session)
check("an expired session can't write", s == 404)

# ---- replays and rate limits
path = f"/api/private/{SLUG}/state"
h = {"Authorization": signed(os.environ["ADMIN_KEY"], "GET", path)}
first, again = call("GET", path, None, h)[0], call("GET", path, None, h)[0]
check("a signature is good once", first == 200 and again == 404)
codes = [page("POST", f"/projects/{SLUG}/api/redeem", {"token": "0" * 64}, xff="203.0.113.9, 10.9.9.9")[0] for _ in range(12)]
check("sign-in attempts are limited per address", codes[:10] == [404] * 10 and codes[10:] == [429, 429], codes)
codes = [page("POST", f"/projects/{SLUG}/api/redeem", {"token": "0" * 64}, xff=f"198.51.100.{i}, 10.9.9.9")[0] for i in range(3)]
check("another address isn't limited by that", codes == [404, 404, 404], codes)
spoof = [page("POST", f"/projects/{SLUG}/api/message", {"text": "x"}, xff=f"1.1.1.{i}, 192.0.2.7, 10.9.9.9")[0] for i in range(62)]
check("writes are limited per address, whatever the visitor puts first", spoof[-1] == 429 and spoof[0] == 404, spoof[-3:])

# ---- nothing private in the log
log = open(os.environ["API_LOG"], "rb").read()
for secret in (token, session, inv2["token"], "Test Project", "Ship the thing", "Please look at the deck"):
    check(f"the log never holds {secret[:6]}...", secret.encode() not in log)
