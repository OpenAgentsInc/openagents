# Realm making behind invites and the auth v2 groundwork (access.c, migrations/0090_realm_invites.sql,
# docs/auth/2026-10-31-realm-invites-and-auth.md). Off (no flag rows), every route answers as before;
# on, only approved creators and admins plan, make or fork a realm. test.py runs this file; it leaves
# both flags off for the files after it.
import tempfile

AC_SIGN = os.environ["SIGN"]
AC_ADMIN = os.environ["ADMIN_KEY"]


def ac_signed(keyfile, method, path, body=None):
    args = [AC_SIGN, "--key", keyfile, method, API + path]
    tmp = None
    if body is not None:
        tmp = tempfile.NamedTemporaryFile(delete=False)
        tmp.write(body); tmp.close()
        args.append(tmp.name)
    auth = subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()
    if tmp: os.unlink(tmp.name)
    return "Nostr " + auth


def ac_key():
    f = tempfile.mktemp()
    pk = subprocess.run([AC_SIGN, "--new", f], capture_output=True, text=True, check=True).stdout.strip()
    return f, pk


ac_n = [0]
def ac_ip():   # each caller its own address, so the per-address limits of other files never bite
    ac_n[0] += 1
    return f"10.9.{ac_n[0] // 250}.{ac_n[0] % 250}, 35.191.0.1"


def ac_req(method, path, obj=None, key=None, player=None, cookie=None, origin=True, raw=None, ip=None, headers=None):
    body = raw if raw is not None else (json.dumps(obj).encode() if obj is not None else None)
    h = {"X-Forwarded-For": ip or ac_ip()}
    if body is not None: h["Content-Type"] = "application/json"
    if origin and method == "POST": h["Origin"] = API
    if key: h["Authorization"] = ac_signed(key, method, path, body)
    if player: h["X-Verse-Player"] = player
    if cookie: h["Cookie"] = "oa_session=" + cookie
    h.update(headers or {})
    s, b, hd = call(method, path, body, h)
    return s, b.decode(errors="replace"), hd


def ac_login(keyfile):   # the words account's login (POST /api/auth/key): the site session's token
    body = b'{"kind":"login"}'
    s, b, hd = ac_req("POST", "/api/auth/key", raw=body, key=keyfile)
    c = hd.get("Set-Cookie", "")
    return c.split("oa_session=", 1)[1].split(";", 1)[0] if s == 200 and "oa_session=" in c else ""


def ac_player():
    s, b, _ = ac_req("POST", "/api/playtest/player", {})
    return json.loads(b)["key"]


def ac_flag(name, on):
    return ac_req("POST", "/api/admin/access/flag", {"name": name, "on": on}, key=AC_ADMIN)


def ac_lines(text):
    return dict(l.split(" ", 1) for l in text.strip().split("\n") if " " in l)


ac_guest = ac_player()
ac_mk, ac_mk_pk = ac_key()
ac_mk_cookie = ac_login(ac_mk)
check("access: a words account logs in for these checks", len(ac_mk_cookie) == 64)

# ---- off: as before
check("access: the migration adds no flag rows (off)", sql("select count(*) from access.flags") == "0")
s, b, _ = ac_req("GET", "/api/playtest/access", player=ac_guest)
check("access off: the page is told to show MAKE A REALM", s == 200 and ac_lines(b) == {"invites": "0", "make": "1", "in": "0", "creator": "0", "asked": "none"}, (s, b))
s, b, _ = ac_req("POST", "/api/playtest/access/redeem", {"code": "0" * 32}, cookie=ac_mk_cookie)
s2, b2, _ = ac_req("POST", "/api/playtest/nothing-here", {"code": "0" * 32}, cookie=ac_mk_cookie)
check("access off: redeem answers as the path did before", s == 404 and (s, b) == (s2, b2), (s, b, s2, b2))
s, b, _ = ac_req("POST", "/api/playtest/access/request", {"note": "a castle"}, cookie=ac_mk_cookie)
check("access off: the waitlist is not there", s == 404, (s, b))
s, b, _ = ac_req("GET", "/api/auth/v2/me", cookie=ac_mk_cookie)
s2, b2, _ = ac_req("GET", "/api/auth/nothing", cookie=ac_mk_cookie)
check("access off: /api/auth/v2 answers as /api/auth did before", s == 404 and b == b2, (s, b, b2))
s, b, _ = ac_req("POST", "/api/playtest/realms?as=lines", {}, player=ac_guest)
check("access off: MAKE IT is not gated (no plan is a 400, not a 403)", s == 400, (s, b))
s, b, _ = ac_req("POST", "/api/playtest/realms/the-grid/fork", {"rules": []}, player=ac_guest)
check("access off: FORK is not gated", s != 403, (s, b))

# ---- the admins' part
s, b, _ = ac_req("GET", "/api/admin/access")
check("access admin: nobody logged in is a 401", s == 401, (s, b))
s, b, _ = ac_req("GET", "/api/admin/access", key=os.environ["OTHER_KEY"])
check("access admin: someone else is a 403", s == 403, (s, b))
s, b, _ = ac_req("GET", "/api/admin/access", key=AC_ADMIN)
check("access admin: the rows start with the flags, both off", s == 200 and b.startswith("F|realm_invites|0\nF|auth_v2|0\n"), (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/flag", {"name": "realm_invites", "on": True}, key=os.environ["OTHER_KEY"])
check("access admin: only an admin turns a flag on", s == 403 and sql("select count(*) from access.flags") == "0", (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/flag", {"name": "nonsense", "on": True}, key=AC_ADMIN)
check("access admin: an unknown flag is a 400", s == 400, (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/invites", {"uses": 2, "days": 7, "note": "from elsewhere"}, key=AC_ADMIN, origin=False, headers={"Origin": "https://evil.example"})
check("access admin: another site's page is refused", s == 403, (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/invites", {"uses": 0}, key=AC_ADMIN)
check("access admin: uses 1 to 1000", s == 400, (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/invites", {"days": 91}, key=AC_ADMIN)
check("access admin: days 1 to 90", s == 400, (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/invites", {"uses": 2, "days": 7, "note": "for | Topher"}, key=AC_ADMIN)
ac_inv = json.loads(b) if s == 200 else {}
ac_code = ac_inv.get("code", "")
check("access admin: an invite's code and link, once", s == 200 and len(ac_code) == 32 and ac_inv["link"] == API + "/invite#code=" + ac_code and ac_inv["uses"] == 2, (s, b))
check("access admin: only the code's hash is kept", sql(f"select count(*) from access.invites where code_sha256 = '{ac_code}'") == "0"
      and sql(f"select count(*) from access.invites where code_sha256 = encode(sha256('{ac_code}'), 'hex')") == "1")
s, b, _ = ac_req("POST", "/api/admin/access/invites", {}, key=AC_ADMIN)
ac_inv1 = json.loads(b)
check("access admin: an invite is one use for 14 days by default", s == 200 and ac_inv1["uses"] == 1 and ac_inv1["days"] == 14, (s, b))
s, b, _ = ac_req("GET", "/api/admin/access", key=AC_ADMIN)
check("access admin: invites are listed, '|' in a note made a space", f"I|{ac_inv['id']}|2|0|" in b and "|live|for   Topher|" in b, b)
ac_exp = sql(f"select projects.chi(expires_ms) from access.invites where id = {ac_inv['id']}")
check("access admin: each invite says when it expires, in Chicago time", f"|live|for   Topher|{ac_exp}\n" in b and len(ac_exp) == 12, (ac_exp, b))

# ---- on
s, b, _ = ac_flag("realm_invites", True)
check("access: an admin turns realm_invites on", s == 200 and json.loads(b) == {"name": "realm_invites", "on": True}, (s, b))
s, b, _ = ac_req("GET", "/api/playtest/access", player=ac_guest)
check("access on: a guest is told it's invite only", s == 200 and ac_lines(b) == {"invites": "1", "make": "0", "in": "0", "creator": "0", "asked": "none"}, (s, b))
s, b, _ = ac_req("GET", "/api/playtest/access", cookie=ac_mk_cookie)
check("access on: a logged-in key that isn't approved", ac_lines(b)["make"] == "0" and ac_lines(b)["in"] == "1", b)
s, b, _ = ac_req("GET", "/api/playtest/access", key=AC_ADMIN)
check("access on: an admin may make realms", ac_lines(b)["make"] == "1" and ac_lines(b)["creator"] == "1", b)

for path, body in [("/api/playtest/realms/plan?as=lines", {"prompt": "a castle"}), ("/api/playtest/realms/pick?as=lines", {"field": "ground", "words": "snow"}),
                   ("/api/playtest/realms/plan/doc?as=lines", {"name": "a.md", "data": "IyBhIGNhc3RsZQ=="}), ("/api/playtest/realms?as=lines", {"plan": 1, "on": []}),
                   ("/api/playtest/realms/the-grid/fork?as=lines", {"rules": []})]:
    s, b, _ = ac_req("POST", path, body, player=ac_guest)
    check(f"access on: {path.split('?')[0]} refuses a guest, 403 invite only", s == 403 and b == "invite only", (s, b))
s, b, _ = ac_req("POST", "/api/playtest/realms", {"plan": 1, "on": []}, player=ac_guest, cookie=ac_mk_cookie)
check("access on: as JSON it says what is needed", s == 403 and json.loads(b) == {"error": "invite only", "need": "invite"}, (s, b))
s, b, _ = ac_req("GET", "/api/playtest/realms/the-grid/fork", player=ac_guest)
check("access on: reading a FORK pane is not gated", s != 403, (s, b))
s, b, _ = ac_req("POST", "/api/playtest/realms?as=lines", {}, key=AC_ADMIN, player=ac_player())
check("access on: an admin passes the gate (no plan: 400)", s == 400, (s, b))

# redeem
s, b, _ = ac_req("POST", "/api/playtest/access/redeem?as=lines", {"code": ac_code}, player=ac_guest)
check("access redeem: a guest must log in first", s == 401 and b == "log in first", (s, b))
s, b, _ = ac_req("POST", "/api/playtest/access/redeem?as=lines", {"code": "f" * 32}, cookie=ac_mk_cookie)
check("access redeem: an unknown code isn't good", s == 404 and b == "that code isn't good", (s, b))
s, b, _ = ac_req("POST", "/api/playtest/access/redeem?as=lines", {"code": ac_code}, cookie=ac_mk_cookie, origin=False, headers={"Origin": "https://evil.example"})
check("access redeem: another site's page is refused", s == 403, (s, b))
check("access redeem: refusals spend nothing", sql(f"select used from access.invites where id = {ac_inv['id']}") == "0")
s, b, _ = ac_req("POST", "/api/playtest/access/redeem?as=lines", {"code": ac_code.upper()}, cookie=ac_mk_cookie)
check("access redeem: a logged-in key becomes a creator", s == 200 and b == "creator 1\n", (s, b))
check("access redeem: the account is kept as a creator by invite", sql(f"select how || ':' || ref from access.creators where who = 'n-{ac_mk_pk}'") == f"invite:{ac_inv['id']}")
check("access redeem: the code is never logged", ac_code not in open(os.environ["API_LOG"]).read())
s, b, _ = ac_req("POST", "/api/playtest/access/redeem?as=lines", {"code": ac_code}, cookie=ac_mk_cookie)
check("access redeem: a creator redeeming again spends no use", s == 200 and sql(f"select used from access.invites where id = {ac_inv['id']}") == "1", (s, b))
s, b, _ = ac_req("GET", "/api/playtest/access", cookie=ac_mk_cookie)
check("access on: the creator is told to show MAKE A REALM", ac_lines(b)["make"] == "1" and ac_lines(b)["creator"] == "1", b)
ac_mk_player = ac_player()
s, b, _ = ac_req("POST", "/api/playtest/realms?as=lines", {}, player=ac_mk_player, cookie=ac_mk_cookie)
check("access on: the creator passes the gate on MAKE IT", s == 400, (s, b))
s, b, _ = ac_req("POST", "/api/playtest/realms/the-grid/fork?as=lines", {"rules": "x"}, player=ac_mk_player, cookie=ac_mk_cookie)
check("access on: the creator passes the gate on FORK", s != 403, (s, b))
s, b, _ = ac_req("POST", "/api/playtest/realms?as=lines", {}, key=ac_mk, player=ac_player())
check("access on: a NIP-98 signature (an agent) is the same account", s == 400, (s, b))
ac_k2, ac_pk2 = ac_key()
ac_c2 = ac_login(ac_k2)
s, b, _ = ac_req("POST", "/api/playtest/access/redeem", {"code": ac_code}, cookie=ac_c2)
check("access redeem: the second of two uses", s == 200 and json.loads(b) == {"creator": True}, (s, b))
ac_k3, ac_pk3 = ac_key()
ac_c3 = ac_login(ac_k3)
s, b, _ = ac_req("POST", "/api/playtest/access/redeem?as=lines", {"code": ac_code}, cookie=ac_c3)
check("access redeem: used up is the same answer as unknown", s == 404 and b == "that code isn't good", (s, b))
s, b, _ = ac_req("POST", f"/api/admin/access/invites/{ac_inv1['id']}/revoke", {}, key=AC_ADMIN)
check("access admin: an invite is revoked", s == 200 and json.loads(b)["state"] == "revoked", (s, b))
s, b, _ = ac_req("POST", "/api/playtest/access/redeem?as=lines", {"code": ac_inv1["code"]}, cookie=ac_c3)
check("access redeem: a revoked code isn't good", s == 404, (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/invites", {}, key=AC_ADMIN)
ac_old = json.loads(b)
sql(f"update access.invites set expires_ms = 1 where id = {ac_old['id']}")
s, b, _ = ac_req("POST", "/api/playtest/access/redeem?as=lines", {"code": ac_old["code"]}, cookie=ac_c3)
check("access redeem: an expired code isn't good", s == 404, (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/invites/999999/revoke", {}, key=AC_ADMIN)
check("access admin: revoking no invite is a 404", s == 404, (s, b))
ac_ip_fixed = "10.200.0.1, 35.191.0.1"
ac_limited = [ac_req("POST", "/api/playtest/access/redeem", {"code": "e" * 32}, cookie=ac_c3, ip=ac_ip_fixed)[0] for _ in range(11)]
check("access redeem: ten tries a minute an address", ac_limited[:10] == [404] * 10 and ac_limited[10] == 429, ac_limited)

# the waitlist
s, b, _ = ac_req("POST", "/api/playtest/access/request?as=lines", {"note": "a castle"}, player=ac_guest)
check("access request: a guest must log in first", s == 401 and b == "log in first", (s, b))
s, b, _ = ac_req("POST", "/api/playtest/access/request?as=lines", {"note": "  "}, cookie=ac_c3)
check("access request: it says what they'd make", s == 400 and b == "say what you'd make", (s, b))
s, b, _ = ac_req("POST", "/api/playtest/access/request?as=lines", {"note": "a castle", "contact": "not mail"}, cookie=ac_c3)
check("access request: a contact is an email", s == 400, (s, b))
s, b, _ = ac_req("POST", "/api/playtest/access/request?as=lines", {"note": "a castle", "contact": "Me@Example.com"}, cookie=ac_c3)
check("access request: asked, waiting", s == 200 and b == "asked waiting\n", (s, b))
s, b, _ = ac_req("POST", "/api/playtest/access/request?as=lines", {"note": "a castle by the sea"}, cookie=ac_c3)
check("access request: asking again rewrites the one request", s == 200 and sql(f"select count(*) || ':' || max(note) from access.requests where who = 'n-{ac_pk3}'") == "1:a castle by the sea", (s, b))
s, b, _ = ac_req("GET", "/api/playtest/access", cookie=ac_c3)
check("access request: the page knows it's waiting", ac_lines(b)["asked"] == "waiting" and ac_lines(b)["make"] == "0", b)
s, b, _ = ac_req("POST", "/api/playtest/access/request?as=lines", {"note": "more"}, cookie=ac_mk_cookie)
check("access request: a creator needn't ask", s == 200 and b == "creator 1\n", (s, b))
ac_rid = sql(f"select id from access.requests where who = 'n-{ac_pk3}'")
s, b, _ = ac_req("GET", "/api/admin/access", key=AC_ADMIN)
check("access admin: the request is listed with its npub", f"R|{ac_rid}|npub1" in b and "|a castle by the sea|me@example.com|waiting|" in b, b)
ac_at = sql(f"select projects.chi(at_ms) from access.requests where id = {ac_rid}")
check("access admin: each request and creator says when", f"|waiting|" in b and b.split(f"R|{ac_rid}|", 1)[1].split("\n", 1)[0].endswith("|" + ac_at)
      and all(l.count("|") == 4 for l in b.split("\n") if l.startswith("C|")), b)
s, b, _ = ac_req("POST", f"/api/admin/access/requests/{ac_rid}/approve", {}, key=os.environ["OTHER_KEY"])
check("access admin: only an admin approves", s == 403, (s, b))
s, b, _ = ac_req("POST", f"/api/admin/access/requests/{ac_rid}/approve", {}, key=AC_ADMIN)
check("access admin: an admin approves a request", s == 200 and json.loads(b)["state"] == "approved", (s, b))
s, b, _ = ac_req("POST", f"/api/admin/access/requests/{ac_rid}/decline", {}, key=AC_ADMIN)
check("access admin: a request is decided once", s == 409, (s, b))
s, b, _ = ac_req("GET", "/api/playtest/access", cookie=ac_c3)
check("access request: approved, the page shows MAKE A REALM", ac_lines(b)["make"] == "1" and ac_lines(b)["asked"] == "approved", b)
ac_k4, ac_pk4 = ac_key()
ac_c4 = ac_login(ac_k4)
ac_req("POST", "/api/playtest/access/request", {"note": "a farm"}, cookie=ac_c4)
ac_rid4 = sql(f"select id from access.requests where who = 'n-{ac_pk4}'")
s, b, _ = ac_req("POST", f"/api/admin/access/requests/{ac_rid4}/decline", {}, key=AC_ADMIN)
check("access admin: a request is declined, and still invite only", s == 200 and ac_lines(ac_req("GET", "/api/playtest/access", cookie=ac_c4)[1])["make"] == "0", (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/requests/999999/approve", {}, key=AC_ADMIN)
check("access admin: no such request is a 404", s == 404, (s, b))
s, b, _ = ac_req("POST", "/api/admin/access/creators", {"who": ac_pk4}, key=AC_ADMIN)
check("access admin: an admin names a key a creator", s == 200 and json.loads(b) == {"who": "n-" + ac_pk4, "creator": True}, (s, b))
s, b, _ = ac_req("POST", f"/api/admin/access/creators/n-{ac_pk4}/remove", {}, key=AC_ADMIN)
check("access admin: and takes it back", s == 200 and ac_lines(ac_req("GET", "/api/playtest/access", cookie=ac_c4)[1])["make"] == "0", (s, b))

# grandfathered guests: a guest's made realm (its world owned by the player, no account)
ac_g2 = ac_player()
ac_gid = sql(f"select id from verse.players where key_sha256 = encode(sha256('{ac_g2}'), 'hex')")
sql(f"insert into verse.worlds (id, owner_player, created_ms) values ('made-ac-test', '{ac_gid}', 1); "
    f"insert into verse.made_realms (slug, world, name, style, ground, time, features, tools, made_ms) values ('ac-test', 'made-ac-test', 'T', 'lowpoly', 'meadow', 'day', '[]', '{{}}', 1)")
s, b, _ = ac_req("POST", "/api/admin/access/grandfather", {}, key=AC_ADMIN)
check("access admin: GRANDFATHER approves everyone who made a realm", s == 200 and json.loads(b)["approved"] >= 1 and sql(f"select how from access.creators where who = '{ac_gid}'") == "grandfather", (s, b))
s, b, _ = ac_req("POST", "/api/playtest/realms?as=lines", {}, player=ac_g2)
check("access on: a grandfathered guest passes the gate by their player key", s == 400, (s, b))
s, b, _ = ac_req("GET", "/api/playtest/access", player=ac_g2)
check("access on: and is told to show MAKE A REALM", ac_lines(b)["make"] == "1", b)
sql("delete from verse.made_realms where slug = 'ac-test'; delete from verse.worlds where id = 'made-ac-test'")

# ---- auth v2
s, b, _ = ac_flag("auth_v2", True)
check("auth v2: an admin turns it on", s == 200, (s, b))
s, b, _ = ac_req("GET", "/api/auth/v2/me")
check("auth v2: me needs a login", s == 401, (s, b))
s, b, _ = ac_req("GET", "/api/auth/v2/me", cookie=ac_mk_cookie)
check("auth v2: me is the key's own account", s == 200 and b.startswith(f"account n-{ac_mk_pk}\nid|nostr|npub1"), (s, b))
ac_k5, ac_pk5 = ac_key()
ac_link = b'{"kind":"link"}'
s, b, _ = ac_req("POST", "/api/auth/v2/link/nostr", raw=ac_link, cookie=ac_mk_cookie)
check("auth v2: linking needs the new key's signature", s == 401, (s, b))
s, b, _ = ac_req("POST", "/api/auth/v2/link/nostr", raw=b'{"kind":"login"}', key=ac_k5, cookie=ac_mk_cookie)
check("auth v2: linking signs exactly {kind: link}", s == 400, (s, b))
ac_k6, _ = ac_key()
s, b, _ = ac_req("POST", "/api/auth/v2/link/nostr", raw=ac_link, key=ac_k6)
check("auth v2: linking needs a session to link into", s == 401, (s, b))
s, b, _ = ac_req("POST", "/api/auth/v2/link/nostr", raw=ac_link, key=ac_k5, cookie=ac_mk_cookie)
check("auth v2: the signed key joins the session's account", s == 200 and json.loads(b) == {"account": "n-" + ac_mk_pk, "linked": True}, (s, b))
s, b, _ = ac_req("GET", "/api/auth/v2/me", key=ac_k5)
check("auth v2: either key is the one account", s == 200 and b.startswith(f"account n-{ac_mk_pk}\n") and b.count("id|nostr|") == 2, (s, b))
s, b, _ = ac_req("POST", "/api/playtest/realms?as=lines", {}, key=ac_k5, player=ac_player())
check("auth v2: the linked key makes realms as its account's creator", s == 400, (s, b))
s, b, _ = ac_req("POST", "/api/auth/v2/link/nostr", raw=ac_link, key=ac_k2, cookie=ac_c4)
check("auth v2: a key that is another account's is refused", s == 409, (s, b))
class AcNoRedirect(urllib.request.HTTPRedirectHandler):   # never follow it to WorkOS
    def redirect_request(self, *a, **k): return None
try:
    urllib.request.build_opener(AcNoRedirect).open(urllib.request.Request(API + "/api/auth/v2/workos/start", headers={"Cookie": "oa_session=" + ac_mk_cookie}))
    s, hd = 200, {}
except urllib.error.HTTPError as e:
    s, hd = e.code, dict(e.headers)
ac_loc = hd.get("Location", "")
check("auth v2: AuthKit start redirects with Google first and a state", s == 302 and ac_loc.startswith("https://api.workos.com/user_management/authorize?response_type=code&client_id=client_test&redirect_uri=")
      and "&provider=GoogleOAuth&state=" in ac_loc and "%2Fapi%2Fauth%2Fv2%2Fworkos%2Fcallback" in ac_loc, (s, ac_loc))
ac_state = ac_loc.split("&state=", 1)[1] if "&state=" in ac_loc else ""
s, b, _ = ac_req("GET", "/api/auth/v2/workos/callback?code=c1&state=" + "0" * 64)
check("auth v2: a callback with an unknown state is refused", s == 400, (s, b))
s, b, _ = ac_req("GET", f"/api/auth/v2/workos/callback?code=c1&state={ac_state}")
check("auth v2: a good state reaches the exchange, which is not built", s == 501, (s, b))
s, b, _ = ac_req("GET", f"/api/auth/v2/workos/callback?code=c1&state={ac_state}")
check("auth v2: a state is spent once", s == 400, (s, b))
check("auth v2: the state is kept only as its hash, bound to the session's key",
      sql(f"select count(*) from auth.workos_states where session_pubkey = '{ac_mk_pk}' and used_ms is not null and state_sha256 = encode(sha256('{ac_state}'), 'hex')") == "1")
ac_wl = json.loads(sql(f"select auth.workos_link('{{\"id\": \"user_01TEST\", \"email\": \"Me@Example.com\"}}', '{ac_mk_pk}')"))
check("auth v2: a WorkOS profile links to the logged-in account and makes a session", ac_wl["status"] == 200 and ac_wl["body"]["account"] == "n-" + ac_mk_pk
      and sql(f"select auth.session('{ac_wl['body']['token']}')") == "n-" + ac_mk_pk, ac_wl)
ac_wl2 = json.loads(sql("select auth.workos_link('{\"id\": \"user_01TEST\"}', NULL)"))
check("auth v2: the same WorkOS login later is the same account", ac_wl2["body"]["account"] == "n-" + ac_mk_pk, ac_wl2)
ac_wl3 = json.loads(sql("select auth.workos_link('{\"id\": \"user_02NEW\", \"email\": \"new@example.com\"}', NULL)"))
check("auth v2: a new WorkOS login is a new account w-...", ac_wl3["body"]["account"].startswith("w-"), ac_wl3)
ac_wl4 = json.loads(sql(f"select auth.workos_link('{{\"id\": \"user_02NEW\"}}', '{ac_mk_pk}')"))
check("auth v2: a WorkOS login that is another account's is refused", ac_wl4["status"] == 409, ac_wl4)
s, b, _ = ac_req("GET", "/api/auth/v2/me", cookie=ac_mk_cookie)
check("auth v2: me lists the WorkOS identity's email", "id|workos|me@example.com\n" in b, b)

# ---- off again: as before, and the approvals kept for when it is turned on
ac_flag("auth_v2", False)
s, b, _ = ac_flag("realm_invites", False)
s2, b2, _ = ac_req("POST", "/api/playtest/realms/plan?as=lines", {"prompt": ""}, player=ac_guest)
check("access off again: a guest is not gated", s == 200 and s2 != 403, (s, b, s2, b2))
s, b, _ = ac_req("GET", "/api/playtest/access", player=ac_guest)
check("access off again: MAKE A REALM shows", ac_lines(b)["make"] == "1" and ac_lines(b)["invites"] == "0", b)
s, b, _ = ac_req("GET", "/api/auth/v2/me", cookie=ac_mk_cookie)
check("access off again: /api/auth/v2 is gone", s == 404, (s, b))
check("access off again: the flags are off and nothing else changed", sql("select count(*) from access.flags where on_") == "0")
