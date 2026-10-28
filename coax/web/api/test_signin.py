# Sign-in (pt-accounts): a NIP-98 signed POST /api/playtest/signin ties the
# browser's player key, its worlds, realm, saves and analytics sessions to the
# pubkey's account. test.py runs this file after test_player.py.


def signin(keyfile, body, key=None, auth=None):
    raw = json.dumps(body).encode()
    h = {"Content-Type": "application/json", "Authorization": auth or nip98(keyfile, "POST", "signin", raw)}
    if key:
        h["X-Verse-Player"] = key
    status, out, _ = call("POST", P + "signin", raw, h)
    return status, json.loads(out)


def track(sid, key=None, n=1):
    h = {"Content-Type": "application/json", "X-Verse-Automation": "api-test"}
    if key:
        h["X-Verse-Player"] = key
    ev = [{"ev": "page-view", "page": "realm", "ct": 1}] * n
    return call("POST", "/api/analytics/events", json.dumps({"sid": sid, "events": ev}).encode(), h)[0]


ann_key = os.environ["ADMIN_KEY"] + ".ann"
ann = subprocess.run([os.environ["SIGN"], "--new", ann_key], capture_output=True, text=True, check=True).stdout.strip()
bob_key = os.environ["ADMIN_KEY"] + ".bob"
bob = subprocess.run([os.environ["SIGN"], "--new", bob_key], capture_output=True, text=True, check=True).stdout.strip()

status, g1 = pt("POST", "player", {})
w1 = pt("POST", "worlds", {}, key=g1["key"])[1]["id"]
status, realm = pt("POST", "planets", {"user": {"id": g1["id"], "name": "Ann"}, "settings": {"name": "Ann's", "kind": "forest"}}, key=g1["key"])
sid1 = "c1111111111111111"
check("a guest's batch with the player key records the player on the session",
      track(sid1, g1["key"]) == 200 and sql(f"select player || ' ' || coalesce(npub, '-') from analytics.sessions where sid='{sid1}'") == g1["id"] + " -")

check("sign-in needs a signature", pt("POST", "signin", {"sid": sid1}, key=g1["key"])[0] == 401)
check("a bad signature is refused", call("POST", P + "signin", b"{}", {"Authorization": "Nostr e30=", "X-Verse-Player": g1["key"]})[0] == 401)
raw1 = json.dumps({"sid": sid1}).encode()
auth1 = nip98(ann_key, "POST", "signin", raw1)
status, done = signin(ann_key, {"sid": sid1}, key=g1["key"], auth=auth1)
check("signing in keeps the guest's key, now the pubkey's", status == 200 and done["signedIn"] and done["npub"] == ann
      and done["player"] == {"key": g1["key"], "id": g1["id"], "account": "n-" + ann}, done)
check("the guest's world and realm moved, and the session",
      done["moved"]["worlds"] == 2 and done["moved"]["sessions"] == 1, done)
check("the same signature is good once", call("POST", P + "signin", raw1, {"Content-Type": "application/json", "Authorization": auth1, "X-Verse-Player": g1["key"]})[0] == 401)
check("the worlds are the account's", sql(f"select count(*) from verse.worlds where owner_account='n-{ann}'") == "2"
      and sorted(pt("GET", "player", key=g1["key"])[1]["worlds"]) == sorted([w1, realm["id"]]))
check("the realm's award is the account's", sql(f"select user_id from verse.awards where planet='{realm['id']}'") == "n-" + ann)
check("the session carries the pubkey", sql(f"select npub from analytics.sessions where sid='{sid1}'") == ann)
track(sid1)
check("later events in that session carry it, even sent as a beacon", sql(f"select npub from analytics.events where sid='{sid1}' order by id desc limit 1") == ann)
sid2 = "c2222222222222222"
track(sid2, g1["key"])
check("a new session with the signed-in key carries it from the start",
      sql(f"select npub || ' ' || player from analytics.sessions where sid='{sid2}'") == ann + " " + g1["id"]
      and sql(f"select acct from analytics.events where sid='{sid2}'") == "n-" + ann)

slot = "progress/haunted-house/main"
status, g2 = pt("POST", "player", {})
g2 = pt("POST", "join", {"email": "ann@example.com", "how": "email"}, key=g2["key"])[1]["player"]
own = g2["account"]
older, newer = save(b"guest save, older"), save(b"guest save, newer")
pt("POST", slot, {"bytes": b64(older), "at": 3000}, key=g2["key"])
pt("POST", slot, {"bytes": b64(newer), "at": 4000}, key=g2["key"])
pt("POST", "progress/haunted-house/side", {"bytes": b64(older), "at": 3000}, key=g2["key"])
status, done = signin(ann_key, {"n": 1}, key=g2["key"])
check("a joined guest's saves move to the pubkey", status == 200 and done["player"]["account"] == "n-" + ann and done["moved"]["saves"] == 2, done)
check("the guest's own account keeps none", sql(f"select count(*) from verse.progress where account='{own}'") == "0")
status, there = pt("GET", slot, key=g2["key"])
check("the save reads back as it was", there == {"current": {"bytes": b64(newer), "at": 4000}, "previous": {"bytes": b64(older), "at": 3000}}, there)
status, done = signin(ann_key, {"n": 2})
check("another browser signing in with the same key gets a fresh key on the account", status == 200 and done["player"]["account"] == "n-" + ann
      and done["player"]["key"] not in (g1["key"], g2["key"]), done)
check("and the same saves", pt("GET", slot, key=done["player"]["key"])[1]["current"]["at"] == 4000)

status, g3 = pt("POST", "player", {})
g3 = pt("POST", "join", {"email": "ann@example.com", "how": "email"}, key=g3["key"])[1]["player"]
pt("POST", slot, {"bytes": b64(older), "at": 2500}, key=g3["key"])
signin(ann_key, {"n": 3}, key=g3["key"])
status, there = pt("GET", slot, key=g3["key"])
check("an older guest save than the account's stays behind it", there["current"]["at"] == 4000 and there["previous"]["at"] == 3000, there)

status, done = signin(bob_key, {"sid": sid1}, key=g1["key"])
check("a key on one pubkey never moves to another: a fresh key, nothing moved", status == 200 and done["player"]["key"] != g1["key"]
      and done["player"]["account"] == "n-" + bob and done["moved"] == {"worlds": 0, "saves": 0, "sessions": 0}, done)
check("the first pubkey keeps its worlds and session", sql(f"select count(*) from verse.worlds where owner_account='n-{ann}'") == "2"
      and sql(f"select npub from analytics.sessions where sid='{sid1}'") == ann)

status, g4 = pt("POST", "player", {})
sql("insert into verse.accounts (id, email, created_ms) values ('e-old', 'old@example.com', 0)")
sql(f"update verse.players set account = 'e-old' where id = '{g4['id']}'")
status, done = signin(bob_key, {"n": 4}, key=g4["key"])
check("a key on an email's account signs in with a fresh key and takes nothing", status == 200 and done["player"]["key"] != g4["key"]
      and done["moved"]["worlds"] == 0 and sql(f"select account from verse.players where id='{g4['id']}'") == "e-old", done)

status, g5 = pt("POST", "player", {})
check("signed-out play is unchanged: a guest still makes worlds and keeps no server saves",
      pt("POST", "worlds", {}, key=g5["key"])[0] == 200 and pt("GET", slot, key=g5["key"])[0] == 403)
