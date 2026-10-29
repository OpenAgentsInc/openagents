# Edit mode for admins in a built-in realm (eg-edit): an admin logged in on the
# site (/login, oa_session) is told so by /api/auth/me (A|1) and publishes the
# realm's edits (POST /api/playtest/world/everglade) from the page; anyone else,
# and a write from another site, is refused. test.py runs this file after
# test_player.py (pt, P).

admin_hex = os.environ["OA_ADMIN_PUBKEYS"].strip()
for tok, pk in (("e" * 64, admin_hex), ("f" * 64, "d" * 64)):
    sql(f"insert into signin.sessions (token_sha256, pubkey, created_ms, expires_ms) values (encode(sha256('{tok}'), 'hex'), '{pk}', 0, 9999999999999) on conflict do nothing")
ADMIN_SITE = {"Cookie": "oa_session=" + "e" * 64, "Origin": API}
OTHER_SITE = {"Cookie": "oa_session=" + "f" * 64, "Origin": API}

s, b, _ = call("GET", "/api/auth/me", headers={"Cookie": ADMIN_SITE["Cookie"]})
check("an admin's /api/auth/me says so (A|1)", s == 200 and b"A|1" in b, (s, b))
s, b, _ = call("GET", "/api/auth/me", headers={"Cookie": OTHER_SITE["Cookie"]})
check("someone else logged in: no A|1", s == 200 and b"A|1" not in b and b"H|" + ("d" * 64).encode() in b, (s, b))
s, b, _ = call("GET", "/api/auth/me")
check("signed out: no viewer at all", s == 401 and b"A|1" not in b, (s, b))

edits = {"everglade": 1, "edits": "eg-edit 1\nt 1 4.50 -10.00 0.79 1.00\n"}
st, g = pt("POST", "player", {})
s, b = pt("POST", "world/everglade", {"doc": edits, "summary": "a guest"}, key=g["key"])
check("a guest can't publish Everglade's edits", s == 403, (s, b))
s, b = pt("POST", "world/everglade", {"doc": edits, "summary": "signed out"})
check("nor can someone signed out", s == 403, (s, b))
s, b = pt("POST", "world/everglade", {"doc": edits, "summary": "not an admin"}, headers=OTHER_SITE)
check("nor someone logged in who isn't an admin", s == 403, (s, b))
s, b = pt("POST", "world/everglade", {"doc": edits, "summary": "cross-site"}, headers={**ADMIN_SITE, "Origin": "https://evil.example"})
check("an admin's cookie on another site's write doesn't count", s == 403, (s, b))
check("nothing was published", pt("GET", "world/everglade")[1] == {"doc": None, "version": 0})

s, b = pt("POST", "world/everglade", {"doc": edits, "summary": "Placed a barrel"}, headers=ADMIN_SITE)
check("an admin logged in on the site publishes Everglade's edits", s == 200 and b == {"version": 1}, (s, b))
s, b = pt("GET", "world/everglade")
check("and everyone gets them, signed out too", s == 200 and b["doc"] == edits and b["version"] == 1, (s, b))
s, b = pt("GET", "world/everglade/owner", headers=ADMIN_SITE)
check("the realm says the admin may edit it", s == 200 and b["you"] is True and b["site"] is True, (s, b))
s, b = pt("GET", "world/everglade/owner", key=g["key"])
check("and a guest that they may not", s == 200 and b["you"] is False and b["site"] is False, (s, b))
s, b = pt("POST", "world/everglade", {"doc": {"everglade": 1, "edits": "eg-edit 1\n"}, "summary": "Took the barrel away"}, headers=ADMIN_SITE)
check("a second publish is the next version", s == 200 and b == {"version": 2}, (s, b))
s, b = pt("POST", "world/everglade/reset", {}, key=g["key"])
check("a guest can't put the designed city back", s == 403, (s, b))
s, b = pt("POST", "world/everglade/reset", {}, headers=OTHER_SITE)
check("nor can someone who isn't an admin", s == 403, (s, b))
s, b = pt("POST", "world/everglade/reset", {}, headers=ADMIN_SITE)
check("the admin puts the designed city back", s == 200 and b == {"reset": True, "was": 2}, (s, b))
check("so everyone sees the plan again", pt("GET", "world/everglade")[1] == {"doc": None, "version": 0})
log = pt("GET", "log/everglade")[1]
check("every edit and the reset are in the realm's log, newest first",
      [e["state"] for e in log] == ["reset", "published", "published"] and log[2]["summary"] == "Placed a barrel", log)
