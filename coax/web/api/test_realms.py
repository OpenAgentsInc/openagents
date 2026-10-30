# A guest's made realm moves to their account when they log in (byo-guest-claim; realms.c,
# migrations/0015_verse_signin.sql, 0048_realm_builder.sql): a guest key makes a realm, logs in
# (the NIP-98 sign-in the /login page's approval runs), and the account owns it, on that browser
# and on a fresh one: the spec says owner, settings and ACCEPT or DECLINE work, and the old guest
# account's key on another browser doesn't. test.py runs this file after test_realm_rights.py
# (b_call, b_player, nip98 and the builder's fake model come from the files before it; the character chat's
# fake model is this file's own, on test_npc.py's port).


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


# A made realm's characters talk (byo-talk; migrations/0061_realm_talk.sql): the character chat
# answers with "realm" the slug while the owner gives TALK; with it off, only the owner.
class TalkModel(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers["Content-Length"]))
        out = json.dumps({"candidates": [{"content": {"role": "model", "parts": [{"text": "I keep the bridge."}]}, "finishReason": "STOP"}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


def talk_checks():
    model = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["NPC_PORT"])), TalkModel)
    threading.Thread(target=model.serve_forever, daemon=True).start()
    owner = b_player()
    st, made = b_call("POST", "/api/playtest/realms", {"board": {"name": "Talking Hollow", "features": [{"feature": "rocks", "on": True, "count": 2}]}}, owner)
    slug = made.get("slug", "talking-hollow")

    def ask(key, realm=slug):
        body = {"npc": {"name": "Old Mott", "role": "keeps the bridge"}, "world": "Talking Hollow", "realm": realm, "question": "Who are you?"}
        h = {"Content-Type": "application/json"}
        if key:
            h["X-Verse-Player"] = key
        st, raw, _ = call("POST", "/api/playtest/npc/ask", json.dumps(body).encode(), h)
        return st, json.loads(raw)

    visitor = b_player()
    sql("delete from verse.npc_day")   # the day's cap (five in test.sh) is test_npc.py's to spend
    b_call("POST", "/api/playtest/realms/" + slug + "/settings", {"tools": {"talk": True}}, owner)
    st, a = ask(visitor)
    check("with TALK on a visitor's question is answered in character", st == 200 and a.get("text"), a)
    check("and kept in the realm's name", sql("select world||'|'||npc from verse.npc_log order by id desc limit 1") == "Talking Hollow|Old Mott")
    b_call("POST", "/api/playtest/realms/" + slug + "/settings", {"tools": {"talk": False}}, owner)
    used = sql("select n from verse.npc_day where day = current_date")
    st, a = ask(visitor)
    check("with TALK off a visitor gets no answer", st == 403, a)
    check("and nothing is spent on the day's cap", sql("select n from verse.npc_day where day = current_date") == used)
    check("nor does someone with no key", ask(None)[0] == 403)
    st, a = ask(owner)
    check("the owner still talks", st == 200 and a.get("text"), a)
    check("a realm that isn't there is a 404", ask(visitor, "no-such-realm")[0] == 404)
    model.shutdown(); model.server_close()


talk_checks()


# A made realm may be flat (byo-painterly; migrations/0062_realm_flat.sql): the style question offers
# it, the planner is told what it looks like, and a board asking for it makes a flat realm.
def flat_checks():
    check("the style question offers low poly, painterly and flat",
          sql("select verse.builder_questions()->'style'->>'words'") == '["lowpoly", "painterly", "flat"]'
          and "flat (bold flat shapes" in sql("select verse.builder_questions()->'style'->>'asks'"))
    check("the planner is told what flat looks like", "painterly (soft, painted), flat (bold flat shapes" in sql("select verse.builder_prompt()"))
    owner = b_player()
    st, made = b_call("POST", "/api/playtest/realms", {"board": {"name": "Flat Fields", "style": "flat", "features": [{"feature": "rocks", "on": True, "count": 2}]}}, owner)
    st, spec = b_call("GET", "/api/playtest/realms/" + made.get("slug", "flat-fields"))
    check("a board asking for flat makes a flat realm", "style flat" in spec, spec)


flat_checks()
