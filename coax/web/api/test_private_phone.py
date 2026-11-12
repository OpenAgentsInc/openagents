# A private project from a logged-in session and from the phone (private.c, projects.api): a session
# of ours (auth.c, /login) whose account's key is a member opens the project page as that member; any
# other session gets the same 404 as anyone; an invite opened on the phone links its key to that
# member; and a linked key acts as its member until the member leaves the document. test.py runs
# this after test_private.py, whose helpers (new_key, signed, page, agent, ...) it uses.
PSLUG = "phone-proj"
CY_KEY, CY = new_key("ph-cy.key")          # a member
CYPHONE_KEY, CYPHONE = new_key("ph-cyphone.key")   # Cy's phone
STRANGER_KEY, STRANGER = new_key("ph-stranger.key")


def phone(keyfile, path, body, xff="10.1.0.9", sign=True, raw=None):
    raw = raw if raw is not None else json.dumps(body, separators=(",", ":")).encode()
    time.sleep(1.05)   # one signed event per second per request: each is good once
    h = {"Content-Type": "application/json", "X-Forwarded-For": xff}
    if sign: h["Authorization"] = signed(keyfile, "POST", path, raw)
    s, b, hd = call("POST", path, raw, h)
    try: return s, json.loads(b), hd
    except ValueError: return s, b, hd


def at_page(path, cookie=None, method="GET", body=None, origin=API, xff="10.1.0.3"):
    h = {"X-Forwarded-For": xff}
    if origin: h["Origin"] = origin
    if cookie: h["Cookie"] = cookie
    raw = None if body is None else json.dumps(body).encode()
    return call(method, path, raw, h)


DOC2 = dict(DOC, slug=PSLUG, title="Phone Project", members=[{"name": "Ada", "pubkey": ADMIN_PUB}, {"name": "Cy", "pubkey": CY}],
            goals=[dict(DOC["goals"][0], agreed_by=["Ada", "Cy"])])
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": 0, "doc": DOC2}, slug=PSLUG)
check("a second fixture project", s == 200, (s, b))

# ---- a logged-in session: its key's member, or nobody
ada_site, stranger_site = login_cookie(ADMIN_PUB), login_cookie(STRANGER)
s, b, _ = at_page(f"/projects/{PSLUG}", cookie=stranger_site)
check("a session whose key is no member gets the locked page", s == 404, s)
s, b, _ = at_page(f"/projects/{PSLUG}/api/state", cookie=stranger_site)
check("and the same 404 as anyone", s == 404 and b == b'{"error": "not found"}', (s, b))
s, b, _ = at_page(f"/projects/{PSLUG}", cookie=ada_site)
check("a member's session opens the project page", s == 200, s)
s, b, _ = at_page(f"/projects/{PSLUG}/api/state", cookie=ada_site)
check("and reads its state as that member", s == 200 and b"Y|Ada|member" in b, (s, b[:80]))
s, b, _ = at_page(f"/projects/{PSLUG}/api/message", cookie=ada_site, method="POST", body={"text": "from the site"})
check("and writes as that member", s == 200 and sql(f"select from_pubkey from projects.messages where slug = '{PSLUG}' and text = 'from the site'") == ADMIN_PUB, (s, b))
s, b, _ = at_page(f"/projects/{PSLUG}/api/message", cookie=ada_site, method="POST", body={"text": "x"}, origin=None)
check("a session's write still needs this site's Origin", s == 403, s)
s, b, _ = at_page(f"/projects/{PSLUG}/api/state", cookie="oa_session=" + "e" * 64)
check("the old sign-in's cookie is nobody here", s == 404, s)
s, b, _ = at_page(f"/projects/{PSLUG}/api/link", cookie=ada_site, method="POST", body={})
check("LINK A PHONE is gone", s != 200 and b"challenge" not in b, (s, b))
s, b, _ = call("GET", "/api/auth/viewer", headers={"Cookie": ada_site})
check("the viewer lists the project", s == 200 and f"J|{PSLUG}".encode() in b and f"H|{ADMIN_PUB}".encode() in b, (s, b))

# ---- an invite opened on the phone links it to that member
s, inv, _ = agent(os.environ["ADMIN_KEY"], "POST", "invite", {"pubkey": CY}, slug=PSLUG)
s, b, _ = phone(CYPHONE_KEY, f"/api/private/{PSLUG}/claim", {"token": inv["token"]}, sign=False)
check("a claim must be signed", s == 403 or s == 404, (s, b))
s, b, _ = phone(os.environ["ADMIN_KEY"], f"/api/private/{PSLUG}/claim", {"token": inv["token"]})
check("a key that is another member can't claim, and the invite stays good", s == 400
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
s, b, _ = agent(CYPHONE_KEY, "GET", "state", slug=SLUG)
check("but only in the project it was linked to", s == 404, s)
s, b, _ = agent(CYPHONE_KEY, "POST", "invite", {"pubkey": CY}, slug=PSLUG)
check("a linked key acts as its member (members invite)", s == 200, (s, b))
cyphone_site = login_cookie(CYPHONE)
s, b, _ = at_page(f"/projects/{PSLUG}/api/state", cookie=cyphone_site)
check("a session of the phone's key reads the project as Cy", s == 200 and b"Y|Cy|member" in b, (s, b[:80]))
s, b, _ = call("GET", "/api/auth/viewer", headers={"Cookie": cyphone_site})
check("its viewer names the member the linked key speaks for", s == 200 and f"\nM|{CY}".encode() in b and f"J|{PSLUG}".encode() in b, (s, b))

# ---- a linked key stops working when its member leaves the document
s, st, _ = agent(os.environ["ADMIN_KEY"], "GET", "state", slug=PSLUG)
DOC3 = dict(DOC2, members=[{"name": "Ada", "pubkey": ADMIN_PUB}], goals=[dict(DOC["goals"][0], agreed_by=["Ada"])])
s, b, _ = agent(os.environ["ADMIN_KEY"], "POST", "push", {"base": st["version"], "doc": DOC3}, slug=PSLUG)
s, b, _ = agent(CYPHONE_KEY, "GET", "state", slug=PSLUG)
check("a removed member's phone is nobody", s == 404, (s, b))
s, b, _ = at_page(f"/projects/{PSLUG}/api/state", cookie=cyphone_site)
check("and so is its session", s == 404, s)

# ---- LOG OUT on the project page ends the project's own cookie; the session stays the site's to end
req = urllib.request.Request(API + f"/projects/{PSLUG}/api/logout", method="POST", data=b"{}",
                             headers={"Origin": API, "Cookie": ada_site, "X-Forwarded-For": "10.1.0.3"})
with urllib.request.urlopen(req) as r:
    s, cookies = r.status, r.headers.get_all("Set-Cookie") or []
check("the project page's logout clears its own cookie and nothing of the site's", s == 200
      and any(c.startswith("oa_project=;") for c in cookies) and not any(c.startswith("__Host-oa_session=") for c in cookies), (s, cookies))
s, b, h = call("POST", "/api/auth/logout", b"{}", {"Content-Type": "application/json", "Origin": API, "Cookie": ada_site})
check("/api/auth/logout ends the session and clears its cookie", s == 200 and "__Host-oa_session=;" in h.get("Set-Cookie", "")
      and at_page(f"/projects/{PSLUG}/api/state", cookie=ada_site)[0] == 404, (s, b, h))
