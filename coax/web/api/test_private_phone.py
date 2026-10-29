# Signing in with the phone (migrations/0019_phone_signin.sql, private.c's
# /api/auth/*): challenges, the phone's signed approval, the page's poll and
# its session, LINK A PHONE, invite links opened on the phone, and that a
# linked key acts as its member. test.py runs this after test_private.py,
# whose helpers (new_key, signed, page, ...) it uses.
PSLUG = "phone-proj"
CY_KEY, CY = new_key("ph-cy.key")          # a member whose phone gets linked
PHONE_KEY, PHONE = new_key("ph-phone.key")  # Ada's phone
CYPHONE_KEY, CYPHONE = new_key("ph-cyphone.key")
STRANGER_KEY, STRANGER = new_key("ph-stranger.key")


def bech32_npub(hexkey):   # BIP-173, the reference algorithm
    cs = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
    data, acc, bits = [], 0, 0
    for b in bytes.fromhex(hexkey):
        acc = (acc << 8) | b; bits += 8
        while bits >= 5:
            bits -= 5; data.append((acc >> bits) & 31)
    if bits: data.append((acc << (5 - bits)) & 31)
    def polymod(v):
        g = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]; chk = 1
        for x in v:
            top = chk >> 25; chk = (chk & 0x1ffffff) << 5 ^ x
            for i in range(5): chk ^= g[i] if (top >> i) & 1 else 0
        return chk
    hrp = "npub"
    pm = polymod([ord(c) >> 5 for c in hrp] + [0] + [ord(c) & 31 for c in hrp] + data + [0] * 6) ^ 1
    return hrp + "1" + "".join(cs[d] for d in data) + "".join(cs[(pm >> 5 * (5 - i)) & 31] for i in range(6))


def site(method, op, body=None, cookie=None, origin=API, xff="10.1.0.1"):
    h = {"X-Forwarded-For": xff}
    if origin: h["Origin"] = origin
    if cookie: h["Cookie"] = "oa_session=" + cookie
    raw = None if body is None else json.dumps(body).encode()
    if raw is not None: h["Content-Type"] = "application/json"
    s, b, hd = call(method, "/api/auth/" + op, raw, h)
    try: return s, json.loads(b), hd
    except ValueError: return s, b, hd


def phone(keyfile, path, body, xff="10.1.0.9", sign=True, raw=None):
    raw = raw if raw is not None else json.dumps(body, separators=(",", ":")).encode()
    time.sleep(1.05)   # one signed event per second per request: each is good once
    h = {"Content-Type": "application/json", "X-Forwarded-For": xff}
    if sign: h["Authorization"] = signed(keyfile, "POST", path, raw)
    s, b, hd = call("POST", path, raw, h)
    try: return s, json.loads(b), hd
    except ValueError: return s, b, hd


def cookie_of(h, name):
    v = h.get("Set-Cookie", "")
    return v.split(name + "=", 1)[1].split(";", 1)[0] if name + "=" in v else ""


check("the npub encoding matches BIP-173", sql(f"select signin.bech32('npub', '{PHONE}')") == bech32_npub(PHONE))

# ---- a sign-in challenge: made by this site's page, approved by the phone, polled once
s, b, _ = site("POST", "challenge", {}, origin=None)
check("a challenge needs this site's Origin", s == 403)
s, b, _ = site("POST", "challenge", {}, origin="https://evil.example")
check("a challenge from another site is refused", s == 403)
s, ch, _ = site("POST", "challenge", {})
check("the page gets a challenge and its own poll token", s == 200 and len(ch["challenge"]) == 64 and len(ch["poll"]) == 64
      and ch["challenge"] != ch["poll"] and ch["expires_in"] == 300 and ch["url"] == API + "/signin#c=" + ch["challenge"], (s, ch))
check("only the challenge's and poll's hashes are kept",
      sql(f"select count(*) from signin.challenges where challenge_sha256 = encode(sha256('{ch['challenge']}'), 'hex') and poll_sha256 = encode(sha256('{ch['poll']}'), 'hex')") == "1"
      and sql(f"select count(*) from signin.challenges where challenge_sha256 = '{ch['challenge']}' or poll_sha256 = '{ch['poll']}'") == "0")
s, b, _ = site("POST", "poll", {"poll": ch["poll"]})
check("before the phone approves, the poll waits", s == 200 and b == {"state": "waiting"}, (s, b))
s, b, _ = site("POST", "poll", {"poll": ch["challenge"]})
check("the challenge itself is not a poll token (the QR alone can't take the session)", s == 404, (s, b))
s, b, _ = phone(PHONE_KEY, "/api/auth/approve", {"challenge": ch["challenge"], "kind": "signin"}, sign=False)
check("an unsigned approval is refused", s == 403, (s, b))
body = json.dumps({"challenge": ch["challenge"], "kind": "signin"}, separators=(",", ":")).encode()
time.sleep(1.05)
args = [SIGN_BIN, "--key", PHONE_KEY, "POST", API + "/api/auth/approve"]   # signed without a payload tag
s, b, _ = call("POST", "/api/auth/approve", body, {"Content-Type": "application/json", "X-Forwarded-For": "10.1.0.9",
               "Authorization": "Nostr " + subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()})
check("an approval must sign its body (payload tag)", s == 404, (s, b))
s, b, _ = phone(PHONE_KEY, "/api/auth/approve", {"challenge": ch["challenge"], "kind": "link"})
check("an approval names the challenge's kind", s == 404, (s, b))
s, b, _ = phone(PHONE_KEY, "/api/auth/approve", {"challenge": "0" * 64, "kind": "signin"})
check("a made-up challenge is not found", s == 404 and b == {"error": "not found"}, (s, b))
s, b, _ = phone(PHONE_KEY, "/api/auth/approve", {"challenge": ch["challenge"], "kind": "signin"})
check("the phone approves the challenge", s == 200 and b == {"ok": True, "kind": "signin"}, (s, b))
s, b, _ = phone(STRANGER_KEY, "/api/auth/approve", {"challenge": ch["challenge"], "kind": "signin"})
check("a challenge is approved once", s == 404, (s, b))
s, b, h = site("POST", "poll", {"poll": ch["poll"]})
sess = cookie_of(h, "oa_session")
setc = h.get("Set-Cookie", "")
check("the poll then signs the page in, bound to the phone's key", s == 200 and b == {"state": "signed-in", "npub": bech32_npub(PHONE)}
      and len(sess) == 64 and sql(f"select pubkey from signin.sessions where token_sha256 = encode(sha256('{sess}'), 'hex')") == PHONE, (s, b))
check("the session cookie is HttpOnly, Secure, SameSite=Lax, site-wide, 30 days",
      all(x in setc for x in ("HttpOnly", "Secure", "SameSite=Lax", "Path=/;", "Max-Age=2592000")), setc)
check("the session token isn't in the answer's body", sess not in json.dumps(b))
s, b, _ = site("POST", "poll", {"poll": ch["poll"]})
check("the poll is good once", s == 404, (s, b))
s, ch2, _ = site("POST", "challenge", {})
sql(f"update signin.challenges set expires_ms = 1 where poll_sha256 = encode(sha256('{ch2['poll']}'), 'hex')")
s, b, _ = phone(PHONE_KEY, "/api/auth/approve", {"challenge": ch2["challenge"], "kind": "signin"})
check("a challenge expires", s == 404 and site("POST", "poll", {"poll": ch2["poll"]})[0] == 404, (s, b))
check("challenges last five minutes", sql("select bool_and(expires_ms - created_ms = 300000) from signin.challenges where expires_ms > 1") == "t")

s, b, _ = site("GET", "me", cookie=sess)
check("me: the session's npub, no projects yet", s == 200 and b == ("K|" + bech32_npub(PHONE) + "\n\n").encode(), (s, b))
check("me without a session is 401", site("GET", "me")[0] == 401 and site("GET", "me", cookie="ab" * 32)[0] == 401)

# ---- a private project, and the phone's session before it is linked
DOC2 = dict(DOC, slug=PSLUG, title="Phone Project", members=[{"name": "Ada", "pubkey": ADMIN_PUB}, {"name": "Cy", "pubkey": CY}],
            goals=[dict(DOC["goals"][0], agreed_by=["Ada", "Cy"])])
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": 0, "doc": DOC2}, slug=PSLUG)
check("a second fixture project", s == 200, (s, b))


def at_page(path, cookie=None, method="GET", body=None, origin=API, xff="10.1.0.3"):
    h = {"X-Forwarded-For": xff}
    if origin: h["Origin"] = origin
    if cookie: h["Cookie"] = cookie
    raw = None if body is None else json.dumps(body).encode()
    return call(method, path, raw, h)


s, b, _ = at_page(f"/projects/{PSLUG}", cookie="oa_session=" + sess)
check("an unlinked phone's session gets the locked page", s == 404, s)
s, b, _ = at_page(f"/projects/{PSLUG}/api/state", cookie="oa_session=" + sess)
check("and the same 404 as anyone", s == 404 and b == b'{"error": "not found"}', (s, b))

# ---- LINK A PHONE: a member signed in on the page links the phone
s, inv, _ = agent(os.environ["ADMIN_KEY"], "POST", "invite", {"pubkey": ADMIN_PUB}, slug=PSLUG)
s, b, h = at_page(f"/projects/{PSLUG}/api/redeem", method="POST", body={"token": inv["token"]})
ada_cookie = cookie_of(h, "oa_project")
s, b, _ = at_page(f"/projects/{PSLUG}/api/link", method="POST", body={})
check("LINK A PHONE needs a member", s == 404, (s, b))
s, b, _ = at_page(f"/projects/{PSLUG}/api/link", cookie="oa_project=" + ada_cookie, method="POST", body={}, origin="https://evil.example")
check("LINK A PHONE from another site is refused", s == 403, (s, b))
s, lk, _ = at_page(f"/projects/{PSLUG}/api/link", cookie="oa_project=" + ada_cookie, method="POST", body={})
lk = json.loads(lk)
check("a member gets a link challenge", s == 200 and lk["url"] == API + "/signin#link=" + lk["challenge"] and len(lk["challenge"]) == 64 and len(lk["poll"]) == 64, (s, lk))
s, b, _ = phone(PHONE_KEY, "/api/auth/approve", {"challenge": lk["challenge"], "kind": "signin"})
check("a link challenge can't be approved as a sign-in", s == 404, (s, b))
agent(os.environ["ADMIN_KEY"], "POST", "agent", {"pubkey": STRANGER, "name": "Helper", "role": "agent"}, slug=PSLUG)
s, b, _ = phone(STRANGER_KEY, "/api/auth/approve", {"challenge": lk["challenge"], "kind": "link"})
check("an agent's key can't be linked to a member", s == 400, (s, b))
s, b, _ = phone(CY_KEY, "/api/auth/approve", {"challenge": lk["challenge"], "kind": "link"})
check("another member's key can't be linked to Ada", s == 400, (s, b))
s, b, _ = phone(PHONE_KEY, "/api/auth/approve", {"challenge": lk["challenge"], "kind": "link"})
check("the phone links to Ada", s == 200 and b == {"ok": True, "kind": "link", "name": "Ada"}, (s, b))
check("the phone's key is Ada's", sql(f"select member || ' ' || how from projects.member_keys where slug = '{PSLUG}' and pubkey = '{PHONE}'") == ADMIN_PUB + " link")
s, b, _ = site("POST", "poll", {"poll": lk["poll"]})
check("the page's poll sees it linked, and makes no session", s == 200 and b == {"state": "linked"}, (s, b))

# ---- the phone's key works for the project page
s, b, _ = at_page(f"/projects/{PSLUG}", cookie="oa_session=" + sess)
check("the phone's site session opens the project page", s == 200, s)
s, b, _ = at_page(f"/projects/{PSLUG}/api/state", cookie="oa_session=" + sess)
check("and reads its state as Ada", s == 200 and b"Y|Ada|member" in b, (s, b[:80]))
s, b, _ = at_page(f"/projects/{PSLUG}/api/message", cookie="oa_session=" + sess, method="POST", body={"text": "from the phone"})
check("and writes as Ada", s == 200 and sql(f"select from_pubkey from projects.messages where slug = '{PSLUG}' and text = 'from the phone'") == ADMIN_PUB, (s, b))
s, b, _ = at_page(f"/projects/{PSLUG}/api/message", cookie="oa_session=" + sess, method="POST", body={"text": "x"}, origin=None)
check("a site-session write still needs this site's Origin", s == 403, s)
s, b, _ = agent(PHONE_KEY, "GET", "state", slug=PSLUG)
check("the phone's signature reads the project as Ada", s == 200 and b["you"]["name"] == "Ada", (s, b))
s, b, _ = agent(PHONE_KEY, "GET", "state", slug=SLUG)
check("but only in the project it was linked to", s == 404, s)
s, b, _ = site("GET", "me", cookie=sess)
check("me lists the project", s == 200 and f"J|{PSLUG}".encode() in b and f"J|{SLUG}".encode() not in b, (s, b))

# ---- an invite opened on the phone links it to that member
s, inv, _ = agent(os.environ["ADMIN_KEY"], "POST", "invite", {"pubkey": CY}, slug=PSLUG)
s, b, _ = phone(CYPHONE_KEY, f"/api/private/{PSLUG}/claim", {"token": inv["token"]}, sign=False)
check("a claim must be signed", s == 403 or s == 404, (s, b))
s, b, _ = phone(PHONE_KEY, f"/api/private/{PSLUG}/claim", {"token": inv["token"]})
check("a key already linked to another member can't claim, and the invite stays good", s == 400
      and sql(f"select used_ms is null from projects.invites where token_sha256 = encode(sha256('{inv['token']}'), 'hex')") == "t", (s, b))
s, b, _ = phone(CYPHONE_KEY, f"/api/private/{SLUG}/claim", {"token": inv["token"]})
check("an invite claims only its project", s == 404, (s, b))
s, b, _ = phone(CYPHONE_KEY, f"/api/private/{PSLUG}/claim", {"token": inv["token"]})
check("the phone claims Cy's invite", s == 200 and b == {"ok": True, "name": "Cy"}, (s, b))
check("and its key is Cy's", sql(f"select member || ' ' || how from projects.member_keys where slug = '{PSLUG}' and pubkey = '{CYPHONE}'") == CY + " invite")
s, b, _ = phone(STRANGER_KEY, f"/api/private/{PSLUG}/claim", {"token": inv["token"]})
check("an invite claims once", s == 404, (s, b))
s, b, _ = agent(CYPHONE_KEY, "GET", "state", slug=PSLUG)
check("the claimed phone reads as Cy", s == 200 and b["you"]["name"] == "Cy", (s, b))
s, b, _ = agent(CYPHONE_KEY, "POST", "invite", {"pubkey": CY}, slug=PSLUG)
check("a linked key acts as its member (members invite)", s == 200, (s, b))

# ---- a linked key stops working when its member leaves the document
s, st, _ = agent(os.environ["ADMIN_KEY"], "GET", "state", slug=PSLUG)
DOC3 = dict(DOC2, members=[{"name": "Ada", "pubkey": ADMIN_PUB}], goals=[dict(DOC["goals"][0], agreed_by=["Ada"])])
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": st["version"], "doc": DOC3}, slug=PSLUG)
s, b, _ = agent(CYPHONE_KEY, "GET", "state", slug=PSLUG)
check("a removed member's phone is nobody", s == 404, (s, b))

# ---- signing out ends the site session
req = urllib.request.Request(API + f"/projects/{PSLUG}/api/logout", method="POST", data=b"{}",
                             headers={"Origin": API, "Cookie": "oa_session=" + sess, "X-Forwarded-For": "10.1.0.3"})
with urllib.request.urlopen(req) as r:
    s, cookies = r.status, r.headers.get_all("Set-Cookie") or []
check("SIGN OUT on the project page ends the site session too", s == 200 and any(c.startswith("oa_session=;") for c in cookies)
      and site("GET", "me", cookie=sess)[0] == 401, (s, cookies))
s, ch3, _ = site("POST", "challenge", {})
phone(PHONE_KEY, "/api/auth/approve", {"challenge": ch3["challenge"], "kind": "signin"})
s, b, h = site("POST", "poll", {"poll": ch3["poll"]})
sess2 = cookie_of(h, "oa_session")
s, b, h = site("POST", "logout", {}, cookie=sess2)
check("/api/auth/logout ends it and clears the cookie", s == 200 and "Max-Age=0" in h.get("Set-Cookie", "") and site("GET", "me", cookie=sess2)[0] == 401)

# ---- the Verse player: /signin's poll signs the browser's player key in with the phone's key (0022)
def poll_as(poll, key):
    h = {"X-Forwarded-For": "10.1.0.21", "Origin": API, "Content-Type": "application/json", "X-Verse-Player": key}
    s, b, hd = call("POST", "/api/auth/poll", json.dumps({"poll": poll}).encode(), h)
    return s, json.loads(b)


def signed_in_page(key, keyfile=PHONE_KEY):
    s, c, _ = site("POST", "challenge", {}, xff="10.1.0.21")
    phone(keyfile, "/api/auth/approve", {"challenge": c["challenge"], "kind": "signin"}, xff="10.1.0.22")
    return poll_as(c["poll"], key)


s, guest = pt("POST", "player", {})
gw = pt("POST", "worlds", {}, key=guest["key"])[1]["id"]
s, c, _ = site("POST", "challenge", {}, xff="10.1.0.21")
check("while the phone hasn't approved, the poll moves no player", poll_as(c["poll"], guest["key"]) == (200, {"state": "waiting"})
      and sql(f"select coalesce(account, '-') from verse.players where id = '{guest['id']}'") == "-")
phone(PHONE_KEY, "/api/auth/approve", {"challenge": c["challenge"], "kind": "signin"}, xff="10.1.0.22")
s, b = poll_as(c["poll"], guest["key"])
check("the poll signs the page's Verse player in with the phone's key: same key, the pubkey's account",
      s == 200 and b["state"] == "signed-in" and b["player"] == {"key": guest["key"], "id": guest["id"], "account": "n-" + PHONE}, (s, b))
check("its worlds moved to that account", sql(f"select owner_account from verse.worlds where id = '{gw}'") == "n-" + PHONE
      and pt("GET", "player", key=guest["key"])[1]["account"] == "n-" + PHONE)
s, b = signed_in_page(guest["key"])
check("signing in again keeps the same player", s == 200 and b["player"]["key"] == guest["key"], (s, b))
s, other = pt("POST", "player", {})
s, b = signed_in_page(other["key"], STRANGER_KEY)
theirs = b["player"]["account"]
check("another phone signs another player in to its own account", s == 200 and b["player"]["key"] == other["key"] and theirs not in ("n-" + PHONE, ""), (s, b))
s, b = signed_in_page(other["key"])
check("a player on another pubkey's account stays there: the page gets a fresh key on the phone's",
      s == 200 and b["player"]["key"] != other["key"] and b["player"]["account"] == "n-" + PHONE
      and pt("GET", "player", key=other["key"])[1]["account"] == theirs, (s, b))
s, b = signed_in_page("not a key!")
check("a malformed player key is ignored", s == 200 and b["state"] == "signed-in" and "player" not in b, (s, b))
s, c, _ = site("POST", "challenge", {}, xff="10.1.0.21")
s, b = poll_as(c["poll"], guest["key"])
check("a sign-in nobody approved never touches the player", b == {"state": "waiting"})

# ---- rate limits and logs
codes = [site("POST", "challenge", {}, xff="203.0.113.77, 10.9.9.9")[0] for _ in range(12)]
check("challenges are limited to 10 a minute per address", codes.count(200) == 10 and codes[-1] == 429, codes)
log = open(os.environ["API_LOG"]).read()
check("no challenge, poll, session or invite token reaches the log",
      all(t not in log for t in (ch["challenge"], ch["poll"], lk["challenge"], lk["poll"], sess, inv["token"])))
