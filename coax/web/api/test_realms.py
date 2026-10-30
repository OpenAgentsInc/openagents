# A guest's made realm moves to their account when they log in (byo-guest-claim; realms.c,
# migrations/0015_verse_signin.sql, 0048_realm_builder.sql): a guest key makes a realm, logs in
# (the NIP-98 sign-in the /login page's approval runs), and the account owns it, on that browser
# and on a fresh one: the spec says owner, settings and ACCEPT or DECLINE work, and the old guest
# account's key on another browser doesn't. test.py runs this file after test_realm_rights.py
# (b_call, b_player, nip98 and the builder's fake model come from the files before it).


def guest_claim_checks():
    tries = [0]

    def log_in(keyfile, key=None):
        tries[0] += 1   # a signature is good once: each log-in signs its own body
        raw = json.dumps({"n": tries[0]}).encode()
        h = {"Content-Type": "application/json", "Authorization": nip98(keyfile, "POST", "signin", raw)}
        if key:
            h["X-Verse-Player"] = key
        st, out, _ = call("POST", "/api/playtest/signin", raw, h)
        return st, json.loads(out)

    who_key = os.environ["ADMIN_KEY"] + ".realm-guest"
    subprocess.run([os.environ["SIGN"], "--new", who_key], capture_output=True, text=True, check=True)
    guest = b_player()
    st, made = b_call("POST", "/api/playtest/realms", {"board": {"name": "Guest Glade", "features": [{"feature": "rocks", "on": True, "count": 3}]}}, guest)
    check("a guest makes a realm", st == 200 and made.get("slug") == "guest-glade", made)
    slug = made.get("slug", "guest-glade")
    check("the guest owns it before logging in", "owner 1" in b_call("GET", "/api/playtest/realms/" + slug, key=guest)[1])

    st, done = log_in(who_key, guest)
    check("logging in keeps the guest's key, now the account's", st == 200 and done["player"]["key"] == guest and done["player"]["account"], done)
    acct = done["player"]["account"]
    check("the made realm's world is the account's", sql("select owner_account from verse.worlds where id = 'made-" + slug + "'") == acct)
    check("on that browser it is still theirs", "owner 1" in b_call("GET", "/api/playtest/realms/" + slug, key=guest)[1])

    st, fresh = log_in(who_key)
    other = fresh.get("player", {}).get("key", "")
    check("a fresh browser logging in to the same account gets its own key", st == 200 and other != guest and fresh["player"]["account"] == acct, fresh)
    check("the account owns the realm there too", "owner 1" in b_call("GET", "/api/playtest/realms/" + slug, key=other)[1])
    st, s = b_call("POST", "/api/playtest/realms/" + slug + "/settings", {"tools": {"build": True}}, other)
    check("the account changes its settings", st == 200 and s["tools"]["build"] is True, s)
    visitor = b_player()
    check("a visitor still may not", b_call("POST", "/api/playtest/realms/" + slug + "/settings", {"tools": {"build": False}}, visitor)[0] == 403)
    st, p = b_call("POST", "/api/playtest/realms/" + slug + "/place", {"thing": "barrel", "x": 2, "z": 1}, visitor)
    check("a visitor's build waits as a card", st == 200 and isinstance(p.get("proposed"), int), p)
    st, cards = b_call("GET", "/api/playtest/realms/" + slug + "/proposals", key=other)
    check("the account sees the card", isinstance(cards, str) and cards.startswith("P %s|" % p.get("proposed")), cards)
    st, d = b_call("POST", "/api/playtest/realms/%s/proposals/%s/accept" % (slug, p.get("proposed")), {}, other)
    check("the account accepts it", st == 200 and d["state"] == "accepted" and "placed barrel 2 1" in b_call("GET", "/api/playtest/realms/" + slug)[1], d)
    st, p2 = b_call("POST", "/api/playtest/realms/" + slug + "/place", {"thing": "crate", "x": -2, "z": 1}, visitor)
    st, d = b_call("POST", "/api/playtest/realms/%s/proposals/%s/decline" % (slug, p2.get("proposed")), {}, guest)
    check("and declines one from the first browser", st == 200 and d["state"] == "declined", d)

    # someone else logging in on a browser that never made it gains nothing
    stranger_key = os.environ["ADMIN_KEY"] + ".realm-stranger"
    subprocess.run([os.environ["SIGN"], "--new", stranger_key], capture_output=True, text=True, check=True)
    st, s2 = log_in(stranger_key, b_player())
    check("another account doesn't own it", "owner 0" in b_call("GET", "/api/playtest/realms/" + slug, key=s2["player"]["key"])[1]
          and b_call("POST", "/api/playtest/realms/" + slug + "/settings", {"tools": {"fly": True}}, s2["player"]["key"])[0] == 403)


guest_claim_checks()
