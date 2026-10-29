# Players, guests, saves and published worlds (pt-player-endpoints): each of the
# demo's player and world endpoints (aethyr-cpp tools/serve.py) against the new
# service. test.py runs this file.
import base64
import tempfile
import zlib

P = "/api/playtest/"


def pt(method, path, body=None, key=None, headers=None):
    h = {"Content-Type": "application/json"}
    if key:
        h["X-Verse-Player"] = key
    h.update(headers or {})
    status, raw, _ = call(method, P + path, None if body is None else json.dumps(body).encode(), h)
    try:
        return status, json.loads(raw)
    except ValueError:
        return status, raw


def nip98(keyfile, method, path, body=None):
    args = [os.environ["SIGN"], "--key", keyfile, method, API + P + path]
    if body is not None:
        f = tempfile.NamedTemporaryFile(delete=False)
        f.write(body)
        f.close()
        args.append(f.name)
    return "Nostr " + subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()


def save(payload, version=2):
    head = b"AESV" + bytes([version, 2]) + payload
    return head + zlib.crc32(head).to_bytes(4, "little")


b64 = lambda b: base64.b64encode(b).decode()

# ---- guests: a player key
status, guest = pt("POST", "player", {}, headers={"X-Verse-Automation": "api-test"})
check("POST player mints a guest's key", status == 200 and len(guest["key"]) >= 20 and guest["id"].startswith("p-") and guest["account"] is None, guest)
check("only the key's hash is kept, with the automation mark",
      sql(f"select count(*)||' '||auto from verse.players where key_sha256 = encode(sha256('{guest['key']}'), 'hex') group by auto") == "1 api-test")
status, again = pt("POST", "player", {}, key=guest["key"])
check("POST player with a key echoes it", status == 200 and again == guest, again)
status, me = pt("GET", "player", key=guest["key"])
check("GET player: id, account, worlds", status == 200 and me == {"id": guest["id"], "account": None, "worlds": []}, me)
check("GET player with an unknown key is a 404", pt("GET", "player", key="x" * 40)[0] == 404)

# ---- worlds: make one, publish, open it by link
check("POST worlds without a key is a 403", pt("POST", "worlds", {})[0] == 403)
status, made = pt("POST", "worlds", {}, key=guest["key"])
wid = made.get("id", "")
check("POST worlds makes a world the player owns", status == 200 and wid.startswith("w-"), made)
check("GET player lists it", pt("GET", "player", key=guest["key"])[1]["worlds"] == [wid])
check("an unpublished world opens empty", pt("GET", "world/" + wid) == (200, {"doc": None, "version": 0}))
doc = {"kind": "haunt", "objects": [{"id": "lamp", "x": 1.5}], "name": "Mine"}
status, pub = pt("POST", "world/" + wid, {"doc": doc, "summary": "a lamp", "user": {"name": "Kiki"}}, key=guest["key"])
check("the owner publishes: version 1", status == 200 and pub == {"version": 1}, pub)
status, seen = pt("GET", "world/" + wid)
check("anyone with the link opens it", status == 200 and seen["doc"] == doc and seen["version"] == 1 and seen["publishedBy"] == "Kiki", seen)
status, other = pt("POST", "player", {})
status, refused = pt("POST", "world/" + wid, {"doc": doc}, key=other["key"])
check("someone else may not publish: 403 with offer", status == 403 and refused.get("offer") is True, refused)
check("nobody known: 403, log in to publish", pt("POST", "world/" + wid, {"doc": doc})[1].get("error") == "log in to publish")
check("a publish needs a document", pt("POST", "world/" + wid, {"summary": "x"}, key=guest["key"])[0] == 400)
check("a world id outside [A-Za-z0-9_-] is not a route", pt("GET", "world/a.b")[0] == 404)

status, own = pt("GET", f"world/{wid}/owner", key=guest["key"])
check("owner: claimed and you", own == {"claimed": True, "you": True, "site": False, "open": False, "name": ""}, own)
check("owner, for someone else: not you", pt("GET", f"world/{wid}/owner", key=other["key"])[1]["you"] is False)
check("only the owner opens it", pt("POST", f"world/{wid}/open", {"open": True}, key=other["key"])[0] == 403)
check("the owner opens it", pt("POST", f"world/{wid}/open", {"open": True}, key=guest["key"]) == (200, {"open": True}))
status, pub2 = pt("POST", "world/" + wid, {"doc": {"kind": "haunt", "objects": []}, "summary": "cleared"}, key=other["key"])
check("an opened world takes anyone known: version 2", status == 200 and pub2 == {"version": 2}, pub2)
check("but not someone unknown", pt("POST", "world/" + wid, {"doc": doc})[0] == 403)
pt("POST", f"world/{wid}/open", {"open": False}, key=guest["key"])
check("closed again", pt("POST", "world/" + wid, {"doc": doc}, key=other["key"])[0] == 403)

check("settings start empty", pt("GET", f"world/{wid}/settings") == (200, {"settings": {}}))
check("bad settings are a 400", pt("POST", f"world/{wid}/settings", {"view": {"view": "side"}}, key=guest["key"])[0] == 400)
check("only the owner sets them", pt("POST", f"world/{wid}/settings", {"view": {"view": "top"}}, key=other["key"])[0] == 403)
status, st = pt("POST", f"world/{wid}/settings", {"view": {"view": "first", "allowToggle": True}}, key=guest["key"])
check("the owner sets the view", status == 200 and st["settings"] == {"view": {"view": "first", "allowToggle": True}}, st)
check("everyone reads them", pt("GET", f"world/{wid}/settings")[1]["settings"]["view"]["view"] == "first")

status, log = pt("GET", "log/" + wid)
check("the log, newest first", status == 200 and [e["version"] for e in log] == [2, 1] and log[1]["summary"] == "a lamp"
      and log[1]["state"] == "published" and log[1]["by"]["player"] == guest["id"] and log[0]["amount"] == 0, log)
check("only the owner resets", pt("POST", f"world/{wid}/reset", {}, key=other["key"])[0] == 403)
check("the owner resets", pt("POST", f"world/{wid}/reset", {}, key=guest["key"]) == (200, {"reset": True, "was": 2}))
check("a reset world opens empty, its settings kept",
      pt("GET", "world/" + wid)[1] == {"doc": None, "version": 0} and pt("GET", f"world/{wid}/settings")[1]["settings"]["view"]["view"] == "first")
check("the reset is logged; every publish keeps its document",
      pt("GET", "log/" + wid)[1][0]["state"] == "reset" and sql(f"select count(doc) from verse.log where world='{wid}'") == "2")

# ---- built-in worlds: the site's
check("a built-in world is the site's: a player may not publish it",
      pt("POST", "world/haunted-house", {"doc": doc}, key=guest["key"])[0] == 403)
body = json.dumps({"doc": doc, "summary": "the site's"}).encode()
status, raw, _ = call("POST", P + "world/haunted-house", body,
                      {"Content-Type": "application/json", "Authorization": nip98(os.environ["ADMIN_KEY"], "POST", "world/haunted-house", body)})
check("an admin's NIP-98 signature publishes it", status == 200 and json.loads(raw) == {"version": 1}, raw)
auth = nip98(os.environ["ADMIN_KEY"], "POST", "world/haunted-house", body)
call("POST", P + "world/haunted-house", body, {"Content-Type": "application/json", "Authorization": auth})
check("a signature is good once", call("POST", P + "world/haunted-house", body, {"Content-Type": "application/json", "Authorization": auth})[0] == 401)
check("a bad signature is a 401", call("GET", P + "player", None, {"Authorization": "Nostr e30="})[0] == 401)
status, raw, _ = call("POST", P + "world/haunted-house", body,
                      {"Content-Type": "application/json", "Authorization": nip98(os.environ["OTHER_KEY"], "POST", "world/haunted-house", body)})
check("someone else's signature does not own it", status == 403)

# ---- the Sandbox (0029_sandbox_open.sql): the site's, open to everyone known
status, own = pt("GET", "world/sandbox/owner", key=guest["key"])
check("the Sandbox is open, and nobody's but the site's", status == 200 and own["open"] is True and own["you"] is False and own["claimed"] is False, own)
status, pub = pt("POST", "world/sandbox", {"doc": doc, "summary": "Built in the Sandbox"}, key=other["key"])
check("anyone with a player key publishes the Sandbox", status == 200 and pub.get("version", 0) >= 1, pub)
check("someone unknown may not", pt("POST", "world/sandbox", {"doc": doc})[0] == 403)
check("and nobody but the site resets it", pt("POST", "world/sandbox/reset", {}, key=other["key"])[0] == 403)
check("every publish is in its log", pt("GET", "log/sandbox")[1][0]["summary"] == "Built in the Sandbox")

# ---- join by email: never verified, so it never opens anyone else's account
slot = "progress/haunted-house/main"
check("a guest keeps no progress on the server", pt("GET", slot, key=guest["key"])[0] == 403)
check("a bad email is a 400", pt("POST", "join", {"email": "not-an-email"}, key=guest["key"])[0] == 400)
status, joined = pt("POST", "join", {"email": " Kiki@Example.com ", "player": "v123", "sid": "s1", "how": "email"}, key=guest["key"])
check("join by email gives the guest's key an account of its own", status == 200 and joined["joined"] and joined["player"]["key"] == guest["key"]
      and joined["player"]["account"].startswith("g-"), joined)
acct = joined["player"]["account"]
check("the join is a playtester row, the email lower-cased",
      sql("select email||' '||player||' '||sid||' '||how from analytics.playtesters where sid='s1'") == "kiki@example.com v123 s1 email")
check("the account keeps no email", sql(f"select coalesce(email, '-') from verse.accounts where id='{acct}'") == "-")
check("no route shows the email", b"kiki@example.com" not in json.dumps([pt("GET", "player", key=guest["key"]), pt("GET", f"world/{wid}/owner")]).encode())
check("the guest's world is still theirs", pt("GET", "player", key=guest["key"])[1] == {"id": guest["id"], "account": acct, "worlds": [wid]})
check("joining again keeps the key and its account",
      pt("POST", "join", {"email": "kiki@example.com", "how": "email"}, key=guest["key"])[1]["player"] == {"key": guest["key"], "id": guest["id"], "account": acct})

check("progress starts empty", pt("GET", slot, key=guest["key"]) == (200, {"current": None, "previous": None}))
s1, s2 = save(b"first save, room 1"), save(b"second save, room 2")
check("a broken save is a 400", pt("POST", slot, {"bytes": b64(s1[:-1] + b"\0"), "at": 1000}, key=guest["key"])[0] == 400)
check("not base64 is a 400", pt("POST", slot, {"bytes": "***", "at": 1000}, key=guest["key"])[0] == 400)
check("a version 3 container is a 400", pt("POST", slot, {"bytes": b64(save(b"x" * 20, 3)), "at": 1000}, key=guest["key"])[0] == 400)
status, kept = pt("POST", slot, {"bytes": b64(s1), "at": 1000}, key=guest["key"])
check("a save is kept", status == 200 and kept == {"current": {"bytes": b64(s1), "at": 1000}, "previous": None}, kept)
status, kept = pt("POST", slot, {"bytes": b64(s2), "at": 2000}, key=guest["key"])
check("the next keeps the last good one", kept == {"current": {"bytes": b64(s2), "at": 2000}, "previous": {"bytes": b64(s1), "at": 1000}}, kept)
status, late = pt("POST", slot, {"bytes": b64(s1), "at": 1500}, key=guest["key"])
check("an older save than the one there is a 409 with what is there", status == 409 and late["current"]["at"] == 2000, late)

status, other_browser = pt("POST", "join", {"email": "kiki@example.com", "how": "email"}, key=other["key"])
check("the same email typed in another browser opens nothing of the first's",
      other_browser["player"]["key"] == other["key"] and other_browser["player"]["account"] not in (None, acct), other_browser)
check("its progress is its own", pt("GET", slot, key=other["key"]) == (200, {"current": None, "previous": None}))
check("nor its worlds", pt("GET", "player", key=other["key"])[1]["worlds"] == [])
sql("insert into verse.accounts (id, email, created_ms) values ('e-legacy', 'legacy@example.com', 0)")
status, legacy = pt("POST", "join", {"email": "legacy@example.com", "how": "email"})
check("an email's account from the demo is never handed out", status == 200 and legacy["player"]["account"] not in (None, "e-legacy"), legacy)
check("erase", pt("POST", slot + "/erase", {}, key=other["key"]) == (200, {"erased": True}))

# ---- a Nostr key is an account too
nbody = b"{}"
status, raw, _ = call("POST", P + "player", nbody, {"Content-Type": "application/json",
                                                   "Authorization": nip98(os.environ["OTHER_KEY"], "POST", "player", nbody)})
check("a NIP-98 signed request is its pubkey's account", status == 200 and json.loads(raw)["account"].startswith("n-"), raw)
check("unknown playtest paths are 404s", pt("GET", "nothing")[0] == 404 and pt("GET", "world/" + wid + "/x/y/z")[0] == 404)
check("a PUT is a 405", call("PUT", P + "player")[0] == 405)

# ---- feedback (pt-feedback): one answer a record, with its session
ans = {"rid": "0123456789abcdef", "q": "howItWent", "a": "  Spooky, but the door stuck.  ", "n": 1, "of": 1,
       "player": guest["id"], "sid": "c0123456789abcdef", "joinedAs": "kiki@example.com"}
check("an answer is kept", pt("POST", "survey/answer", ans, headers={"X-Verse-Automation": "api-test"}) == (200, {"saved": True}))
check("as the demo keeps it: trimmed, with its session and the automation mark",
      sql("select concat_ws('|', a #>> '{}', sid, q, n || '/' || \"of\", auto, skip::text) from analytics.survey_answers where rid='0123456789abcdef' and q='howItWent'")
      == "Spooky, but the door stuck.|c0123456789abcdef|howItWent|1/1|api-test|false", sql("select concat_ws('|', a #>> '{}', sid, q, n || '/' || \"of\", auto, skip::text) from analytics.survey_answers where q='howItWent'"))
check("choices are kept as a list", pt("POST", "survey/answer", {**ans, "q": "liked", "a": ["walking", "x" * 200, 3]})[0] == 200
      and sql("select a::text from analytics.survey_answers where q='liked'") == '["walking", "' + "x" * 80 + '"]')
check("an empty answer is a skip", pt("POST", "survey/answer", {**ans, "q": "email", "a": "  "})[0] == 200
      and sql("select skip::text || coalesce(a::text, '-') from analytics.survey_answers where q='email'") == "true-")
check("a bad respondent or question is a 400",
      pt("POST", "survey/answer", {**ans, "rid": "nope"})[0] == 400 and pt("POST", "survey/answer", {**ans, "q": "1bad"})[0] == 400)
check("an answer that is an object is a 400", pt("POST", "survey/answer", {**ans, "a": {"x": 1}})[0] == 400)
check("the whole form is kept too", pt("POST", "survey", {"fun": 5, "email": "x@y.z"}) == (200, {})
      and sql("select answers->>'fun' from analytics.surveys order by at desc limit 1") == "5")
check("a GET is a 405", pt("GET", "survey/answer")[0] == 405)
status, raw, _ = call("GET", reports, headers={"Authorization": "Nostr " + signed(os.environ["ADMIN_KEY"], "GET", API + reports)})
section = raw.decode().split("#SURVEY ANSWERS|")[-1].split("\n#")[0]
check("the dashboard shows each answer with its session, which opens",
      status == 200 and section.startswith("s|SESSION|WHEN|QUESTION|ANSWER") and "\nc0123456789abcdef|" in section
      and "|howItWent|Spooky, but the door stuck." in section, section[:300])

# ---- offered changes (pt-change-offers): anyone known offers, the owner accepts or declines
status, owner = pt("POST", "player", {})
status, visitor = pt("POST", "player", {})
status, made = pt("POST", "worlds", {}, key=owner["key"])
ow = made["id"]
base = {"kind": "verse.haunt.save", "bytes": "AAAA"}
pt("POST", "world/" + ow, {"doc": base, "summary": "made it", "user": {"name": "Ann"}}, key=owner["key"])
change = {"added": [{"list": "objects", "item": {"id": "placed-1", "name": "Ghost"}}]}
proposed = {"kind": "verse.haunt.save", "bytes": "BBBB"}
check("nobody known may not offer", pt("POST", "change/" + ow, {"delta": change})[0] == 403)
check("an offer needs a change", pt("POST", "change/" + ow, {"delta": {"added": []}}, key=visitor["key"])[0] == 400)
check("a price is whole sats, 0 to 1,000,000", pt("POST", "change/" + ow, {"delta": change, "price": 1.5}, key=visitor["key"])[0] == 400
      and pt("POST", "change/" + ow, {"delta": change, "price": -1}, key=visitor["key"])[0] == 400)
status, off = pt("POST", "change/" + ow, {"delta": change, "doc": proposed, "summary": "Added Ghost.", "user": {"name": "Bo"}}, key=visitor["key"])
check("a second session offers a change: it waits", status == 200 and off["state"] == "waiting" and len(off["id"]) == 12, off)
status, off2 = pt("POST", "change/" + ow, {"delta": change, "doc": {"kind": "verse.haunt.save", "bytes": "CCCC"}, "summary": "Added another."}, key=visitor["key"])
check("the visitor may not accept it", pt("POST", f"change/{ow}/{off['id']}/accept", {}, key=visitor["key"])[0] == 403)
check("an unknown offer is a 404", pt("POST", f"change/{ow}/nope/accept", {}, key=owner["key"])[0] == 404)
check("a GET on a change is a 405", pt("GET", "change/" + ow, key=visitor["key"])[0] == 405)
check("the owner declines one", pt("POST", f"change/{ow}/{off2['id']}/decline", {}, key=owner["key"]) == (200, {"state": "declined"}))
check("a declined offer changes nothing", pt("GET", "world/" + ow)[1]["doc"] == base and pt("GET", "world/" + ow)[1]["version"] == 1)
check("a decided offer stays decided", pt("POST", f"change/{ow}/{off2['id']}/accept", {}, key=owner["key"])[0] == 409)
check("the owner accepts the other", pt("POST", f"change/{ow}/{off['id']}/accept", {}, key=owner["key"]) == (200, {"state": "accepted", "version": 2}))
status, seen = pt("GET", "world/" + ow)
check("the world shows it after a reload", seen["doc"] == proposed and seen["version"] == 2 and seen["publishedBy"] == "Bo", seen)
status, log = pt("GET", "log/" + ow)
check("the log lists every edit, newest first",
      [e["state"] for e in log] == ["declined", "accepted", "published"] and log[0]["id"] == off2["id"]
      and log[1]["id"] == off["id"] and log[1]["version"] == 2 and log[1]["who"] == "Bo" and log[1]["delta"] == change
      and log[1]["by"]["player"] == visitor["id"] and log[1]["guest"] is True and log[2]["version"] == 1, log)
check("offers keep no world in the log", all("doc" not in e for e in log))
status, raw, _ = call("GET", "/api/log/" + ow)
check("the legacy /api/log/<id> answers the same", status == 200 and json.loads(raw) == log)
status, raw, _ = call("POST", "/api/change/" + ow, json.dumps({"delta": change, "summary": "legacy"}).encode(),
                      {"Content-Type": "application/json", "X-Verse-Player": visitor["key"]})
legacy_id = json.loads(raw)["id"]
check("the legacy /api/change/<id> offers too", status == 200 and json.loads(raw)["state"] == "waiting", raw)
check("an offer without its world needs one to accept",
      pt("POST", f"change/{ow}/{legacy_id}/accept", {}, key=owner["key"])[0] == 400
      and pt("POST", f"change/{ow}/{legacy_id}/accept", {"doc": proposed}, key=owner["key"]) == (200, {"state": "accepted", "version": 3}))
check("a built-in world's offers are the site's to decide",
      pt("POST", "change/haunted-house", {"delta": change}, key=visitor["key"])[0] == 200
      and pt("POST", f"change/haunted-house/{pt('GET', 'log/haunted-house')[1][0]['id']}/accept", {"doc": proposed}, key=owner["key"])[0] == 403)

# ---- a player's realm (pt-planets): award, set up, reopen; the galaxy lists it
status, g = pt("POST", "player", {})
_, before = pt("GET", "planets")
check("making a planet needs a player key", pt("POST", "planets", {"user": {"id": g["id"], "name": "Ann"}, "settings": {}})[0] == 403)
check("and the user and settings", pt("POST", "planets", {"user": {"id": "a b"}, "settings": {}}, key=g["key"])[0] == 400
      and pt("POST", "planets", {"user": {"id": g["id"]}}, key=g["key"])[0] == 400)
settings = {"name": "  Moonbog  ", "kind": "cemetery", "art": "Ink", "cam": "Top down", "cycle": "Always night"}
status, planet = pt("POST", "planets", {"user": {"id": g["id"], "name": "Ann"}, "settings": settings}, key=g["key"])
check("a guest is awarded a realm and sets it up", status == 200 and planet["id"].startswith("planet-") and planet["name"] == "Moonbog"
      and planet["owner"] == "Ann" and planet["kind"] == "cemetery" and planet["art"] == "Ink" and planet["cam"] == "Top down"
      and planet["cycle"] == "Always night" and planet["n"] > before["n"] and "ownerId" not in planet and planet["made"] == planet["at"], planet)
pid = planet["id"]
check("the award is the same planet, made once", pt("POST", "award", {"user": g["id"]}) == (200, {"planet": pid}))
check("an award for someone new is theirs", pt("POST", "award", {"user": "name-bo-123"})[1]["planet"].startswith("planet-nameb"))
status, mine = pt("GET", "planets/mine", key=g["key"])
check("it reopens after a reload: planets/mine", status == 200 and mine == {"planet": planet, "invited": False, "saved": None}, mine)
check("it is a world its player owns", pt("GET", f"world/{pid}/owner", key=g["key"])[1]["you"] is True and pid in pt("GET", "player", key=g["key"])[1]["worlds"])
status, fresh = pt("GET", "planets?after=" + str(before["n"]))
check("the galaxy lists the new realm", status == 200 and fresh["planets"] == [planet] and fresh["n"] == planet["n"], fresh)
check("one planet by id", pt("GET", "planets/" + pid) == (200, planet) and pt("GET", "planets/nope")[0] == 404)
status, again = pt("POST", "planets", {"user": {"id": g["id"], "name": "Ann"}, "settings": {"name": "", "kind": "moon", "art": "Neon"}}, key=g["key"])
check("set up again: the same planet, a later number, the defaults where a choice isn't one", status == 200 and again["id"] == pid
      and again["n"] > planet["n"] and again["name"] == "My planet" and again["kind"] == "forest" and again["art"] == "Low poly"
      and again["made"] == planet["made"], again)
check("after its number the galaxy sees nothing new", pt("GET", "planets?after=" + str(again["n"]))[1]["planets"] == [])
status, thief = pt("POST", "player", {})
check("someone else can't set it up", pt("POST", "planets", {"user": {"id": g["id"], "name": "X"}, "settings": settings}, key=thief["key"]) [0] == 403)
check("nobody's planet: planets/mine is empty, keeping is a 404",
      pt("GET", "planets/mine", key=thief["key"])[1] == {"planet": None, "invited": False, "saved": None}
      and pt("POST", "planets/mine", {"invited": True}, key=thief["key"])[0] == 404)
check("a saved copy must be a world record", pt("POST", "planets/mine", {"doc": [1]}, key=g["key"])[0] == 400)
status, kept = pt("POST", "planets/mine", {"doc": {"kind": "verse.world.v1", "id": pid}, "invited": True}, key=g["key"])
check("the owner keeps a copy and the invite", status == 200 and kept["id"] == pid and kept["invited"] is True and kept["at"] > 0, kept)
status, mine = pt("GET", "planets/mine", key=g["key"])
check("and gets them back", mine["invited"] is True and mine["saved"]["doc"] == {"kind": "verse.world.v1", "id": pid}, mine)
pt("POST", "planets/mine", {"doc": None}, key=g["key"])
check("a null doc drops the copy, the invite stays", pt("GET", "planets/mine", key=g["key"])[1]["saved"] is None
      and pt("GET", "planets/mine", key=g["key"])[1]["invited"] is True)
status, raw, _ = call("GET", "/api/planets?after=" + str(before["n"]))
check("legacy /api/planets answers like the playtest's", status == 200 and json.loads(raw)["planets"][-1]["id"] == pid, raw)
status, raw, _ = call("POST", "/api/award", json.dumps({"user": g["id"]}).encode(), {"Content-Type": "application/json"})
check("legacy /api/award", status == 200 and json.loads(raw) == {"planet": pid}, raw)
status, raw, _ = call("GET", "/api/planets/mine", headers={"X-Verse-Player": g["key"]})
check("legacy /api/planets/mine", status == 200 and json.loads(raw)["planet"]["id"] == pid, raw)
check("legacy paths stop at what moved", call("GET", "/api/planetsx")[0] == 404 and call("GET", "/api/world/x")[0] == 404)
