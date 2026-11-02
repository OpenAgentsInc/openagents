# Rooms (pt-presence-chat): presence and chat. test.py runs this file.
import time


def mint():
    status, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
    return json.loads(raw)


def room_call(method, path, body=None, key=None):
    headers = {"Content-Type": "application/json"}
    if key:
        headers["X-Verse-Player"] = key
    status, raw, _ = call(method, path, json.dumps(body).encode() if body is not None else None, headers)
    return status, json.loads(raw)


ann, bo = mint(), mint()
R = "/api/playtest/presence/haunted-house"
status, a = room_call("POST", R, {"x": 1.5, "y": 0, "z": -2, "yaw": 0.5, "after": 0}, ann["key"])
check("a player comes into a room and is alone there", status == 200 and a["others"] == [] and a["you"]["id"] == ann["id"]
      and a["you"]["name"] == "Guest " + ann["id"][2:6].upper(), a)
status, b = room_call("POST", R, {"x": 3, "z": 4, "yaw": 1, "after": 0}, bo["key"])
check("the next one sees the first, where they are", status == 200 and len(b["others"]) == 1 and b["others"][0]["id"] == ann["id"]
      and b["others"][0]["x"] == 1.5 and b["others"][0]["z"] == -2 and b["others"][0]["yaw"] == 0.5, b)
status, a = room_call("POST", R, {"x": 1.6, "z": -2, "after": 0}, ann["key"])
check("and the first sees them back", status == 200 and [o["id"] for o in a["others"]] == [bo["id"]], a)
check("another room is another room", room_call("POST", "/api/playtest/presence/w-elsewhere", {}, ann["key"])[1]["others"] == [])
check("no player key, no room", room_call("POST", R, {})[0] == 403 and room_call("POST", R, {}, "x" * 40)[0] == 403)
check("a room's name is a world id", room_call("POST", "/api/playtest/presence/a.b", {}, ann["key"])[0] == 404)
check("silly numbers are zero", room_call("POST", R, {"x": 1e30, "z": "far", "after": 0}, bo["key"])[0] == 200
      and sql(f"select x || ' ' || z from verse.room_here where player = '{bo['id']}' and room = 'haunted-house'") == "0 0")

C = "/api/playtest/chat/haunted-house"
status, s = room_call("POST", C, {"text": "  hello\nthere  ", "sid": "cabc123"}, ann["key"])
check("a line said in the room", status == 200 and s["n"] >= 1, s)
first = s["n"]
status, m = room_call("GET", C + "?after=%d" % (first - 1))
check("everyone reads it, with who said it", status == 200 and m["messages"][0]["text"] == "hello there"
      and m["messages"][0]["name"] == "Guest " + ann["id"][2:6].upper() and m["last"] == first, m)
status, p = room_call("POST", R, {"after": first - 1}, bo["key"])
check("presence brings the lines after the last one seen, marked whose", status == 200 and [x["text"] for x in p["messages"]] == ["hello there"]
      and p["messages"][0]["mine"] is False and p["last"] == first, p)
check("and the speaker's own are marked theirs", room_call("POST", R, {"after": first - 1}, ann["key"])[1]["messages"][0]["mine"] is True)
status, s = room_call("POST", C, {"text": "x" * 2000}, bo["key"])
check("a line is at most 500 characters", status == 200 and sql(f"select length(text) from verse.room_chat where room = 'haunted-house' and n = {s['n']}") == "500", s)
check("an empty line is refused", room_call("POST", C, {"text": "   "}, bo["key"])[0] == 400)
check("a guest without a key can't speak", room_call("POST", C, {"text": "hi"})[0] == 403)
for i in range(4):
    room_call("POST", C, {"text": "again %d" % i}, bo["key"])
check("five lines in ten seconds, then a pause", room_call("POST", C, {"text": "too fast"}, bo["key"])[0] == 429)
check("the sid is kept with the line", sql(f"select sid from verse.room_chat where room = 'haunted-house' and n = {first}") == "cabc123")
check("the demo's paths answer too", room_call("GET", "/api/chat/haunted-house?after=0")[0] == 200
      and room_call("POST", "/api/presence/haunted-house", {}, ann["key"])[0] == 200)
status, m = room_call("GET", C + "?after=0")
check("a read is the last 50 at most", status == 200 and len(m["messages"]) <= 50)

# A room holds 16: the 17th is told it's full; someone already in stays in.
crowd = [mint() for _ in range(14)]
for c in crowd:
    room_call("POST", "/api/playtest/presence/full-room", {}, c["key"])
room_call("POST", "/api/playtest/presence/full-room", {}, ann["key"])
room_call("POST", "/api/playtest/presence/full-room", {}, bo["key"])
late = mint()
check("a room holds 16 at once", room_call("POST", "/api/playtest/presence/full-room", {}, late["key"]) == (429, {"error": "the room is full"}))
check("those in it stay", room_call("POST", "/api/playtest/presence/full-room", {}, ann["key"])[0] == 200
      and len(room_call("POST", "/api/playtest/presence/full-room", {}, ann["key"])[1]["others"]) == 15)
sql("update verse.room_here set seen_ms = seen_ms - 7000 where room = 'full-room' and player = '%s'" % bo["id"])
check("someone gone six seconds isn't there any more, and frees a place",
      room_call("POST", "/api/playtest/presence/full-room", {}, late["key"])[0] == 200
      and bo["id"] not in [o["id"] for o in room_call("POST", "/api/playtest/presence/full-room", {}, ann["key"])[1]["others"]])

# Who walks a room, for the homepage (0069): GET needs no key and joins nothing; automation (its mark,
# a headless browser) is seen only by its own player, in GET and in the others a POST answers.
G = "/api/playtest/presence/grid-test"
walker, check_bot = mint(), mint()
room_call("POST", G, {"x": 4, "z": -3, "yaw": 1}, walker["key"])
status, raw, _ = call("POST", G, json.dumps({"x": 9, "z": 9}).encode(), {"Content-Type": "application/json", "X-Verse-Player": check_bot["key"], "X-Verse-Automation": "qa-1"})
check("automation in a room still sees the people there", status == 200 and [o["id"] for o in json.loads(raw)["others"]] == [walker["id"]], raw)
status, g = room_call("GET", G)
check("anyone can see who walks a room, and automation isn't among them", status == 200 and g["count"] == 1
      and [(o["id"], o["x"], o["z"]) for o in g["others"]] == [(walker["id"], 4, -3)], g)
check("a person in the room doesn't see the automation", [o["id"] for o in room_call("POST", G, {"x": 4, "z": -3}, walker["key"])[1]["others"]] == [], G)
check("automation sees itself when it asks", check_bot["id"] in [o["id"] for o in room_call("GET", G, None, check_bot["key"])[1]["others"]])
status, raw, _ = call("POST", G, b"{}", {"Content-Type": "application/json", "X-Verse-Player": check_bot["key"], "User-Agent": "Mozilla/5.0 HeadlessChrome/131.0"})
check("a headless browser is automation too", status == 200 and check_bot["id"] not in [o["id"] for o in room_call("GET", G)[1]["others"]])
# A check's sessions that pair (0074): automation marked "pair-..." sees the others of that same mark,
# and nobody else does; a cookie mark pairs as the header does.
PAIR_ROOM = "/api/playtest/presence/pair-test"
pa, pb, pc, person = mint(), mint(), mint(), mint()
def pair_post(key, headers, x):
    status, raw, _ = call("POST", PAIR_ROOM, json.dumps({"x": x, "z": 1}).encode(), {"Content-Type": "application/json", "X-Verse-Player": key, **headers})
    return [(o["id"], o["x"]) for o in json.loads(raw)["others"]]
pair_post(person["key"], {}, 0)
pair_post(pa["key"], {"X-Verse-Automation": "pair-ff-1"}, 1)
check("a paired check sees the other of its pair, and the people", sorted(pair_post(pb["key"], {"Cookie": "a=1; verse-automation=pair-ff-1"}, 2)) == sorted([(person["id"], 0), (pa["id"], 1)]))
check("and the first sees the second move", (pb["id"], 2) in pair_post(pa["key"], {"X-Verse-Automation": "pair-ff-1"}, 1))
check("another pair doesn't see them", pair_post(pc["key"], {"X-Verse-Automation": "pair-ff-2"}, 3) == [(person["id"], 0)])
check("a mark that isn't a pair doesn't see them", pair_post(pc["key"], {"X-Verse-Automation": "qa-1"}, 3) == [(person["id"], 0)])
check("a person doesn't see a pair", pair_post(person["key"], {}, 0) == [])
check("nor does the homepage", [o["id"] for o in room_call("GET", PAIR_ROOM)[1]["others"]] == [person["id"]])
check("a pair's GET sees its pair", sorted(o["id"] for o in json.loads(call("GET", PAIR_ROOM, None, {"X-Verse-Player": pa["key"], "X-Verse-Automation": "pair-ff-1"})[1])["others"]) == sorted([person["id"], pa["id"], pb["id"]]))
check("a GET joins nobody",room_call("GET", "/api/playtest/presence/nobody-here") == (200, {"others": [], "count": 0}))

# A made realm's room and an agent's seat in it (0108; docs/verse/realm-seat.md slice 4). A seat's
# presence is signed (NIP-98): everyone sees it, {"agent": true} and its account's name; its own page
# (named in "page") doesn't. An unsigned "seat" mark is plain automation. A private realm's room is
# 404 to all but its owner and site admins.
import tempfile


def seat_post(room, body, keyfile=None, headers=None):
    raw = json.dumps(body).encode()
    h = {"Content-Type": "application/json", **(headers or {})}
    if keyfile:
        f = tempfile.NamedTemporaryFile(delete=False)
        f.write(raw)
        f.close()
        sig = subprocess.run([os.environ["SIGN"], "--key", keyfile, "POST", API + "/api/playtest/presence/" + room, f.name],
                             capture_output=True, text=True, check=True).stdout.strip()
        h["Authorization"] = "Nostr " + sig
    status, out, _ = call("POST", "/api/playtest/presence/" + room, raw, h)
    return status, json.loads(out)


owner, visitor, page = mint(), mint(), mint()
for slug, vis in (("seat-green", "unlisted"), ("seat-secret", "private")):
    sql(f"insert into verse.worlds (id, owner_player, created_ms) values ('made-{slug}', '{owner['id']}', verse.now_ms())")
    sql(f"insert into verse.made_realms (slug, world, name, style, ground, time, features, tools, made_ms, visibility) "
        f"values ('{slug}', 'made-{slug}', 'S', 'lowpoly', 'meadow', 'day', '{{}}', '{{}}', verse.now_ms(), '{vis}')")
GR = "made-seat-green"
room_call("POST", "/api/playtest/presence/" + GR, {"x": 1, "z": 2}, visitor["key"])
status, seat = seat_post(GR, {"x": 5, "z": 6, "yaw": 1.5, "page": page["id"]}, os.environ["OTHER_KEY"])
check("a signed presence is an agent's seat, under its account", status == 200 and seat["you"]["agent"] is True
      and [o["id"] for o in seat["others"]] == [visitor["id"]], seat)
status, v = room_call("POST", "/api/playtest/presence/" + GR, {"x": 1, "z": 2}, visitor["key"])
agents = [o for o in v["others"] if o.get("agent")]
check("a person in the realm sees the seat, flagged agent, where it stands", status == 200 and len(agents) == 1
      and agents[0]["id"] == seat["you"]["id"] and agents[0]["name"] == seat["you"]["name"] and agents[0]["x"] == 5, v)
status, raw, _ = call("POST", "/api/playtest/presence/" + GR, b"{}", {"Content-Type": "application/json", "X-Verse-Player": page["key"], "X-Verse-Automation": "realm-seat"})
check("the seat's own page sees the people, not its own agent", status == 200 and [o["id"] for o in json.loads(raw)["others"]] == [visitor["id"]], raw)
check("the homepage counts the seat", seat["you"]["id"] in [o["id"] for o in room_call("GET", "/api/playtest/presence/" + GR)[1]["others"]])
forged = mint()
call("POST", "/api/playtest/presence/" + GR, b"{}", {"Content-Type": "application/json", "X-Verse-Player": forged["key"], "X-Verse-Automation": "seat"})
check("an unsigned seat mark is automation, unseen", forged["id"] not in [o["id"] for o in room_call("POST", "/api/playtest/presence/" + GR, {}, visitor["key"])[1]["others"]])
sql(f"update verse.room_here set seen_ms = seen_ms - 20000 where room = '{GR}' and player = '{seat['you']['id']}'")
check("a seat stays seen through a long action (40 s, not 6)", seat["you"]["id"] in [o["id"] for o in room_call("GET", "/api/playtest/presence/" + GR)[1]["others"]])
check("a seat leaves at once", seat_post(GR, {"gone": True}, os.environ["OTHER_KEY"]) == (200, {"gone": True})
      and seat["you"]["id"] not in [o["id"] for o in room_call("GET", "/api/playtest/presence/" + GR)[1]["others"]])
check("a forged signature is refused, not taken as the player key", seat_post(GR, {}, None, {"Authorization": "Nostr e30=", "X-Verse-Player": visitor["key"]})[0] == 401)
SR = "made-seat-secret"
check("a private realm's room is 404 to a visitor", room_call("POST", "/api/playtest/presence/" + SR, {}, visitor["key"])[0] == 404
      and room_call("GET", "/api/playtest/presence/" + SR)[0] == 404 and room_call("GET", "/api/playtest/chat/" + SR + "?after=0")[0] == 404)
check("and to another account's seat", seat_post(SR, {}, os.environ["OTHER_KEY"])[0] == 404)
check("its owner is in it", room_call("POST", "/api/playtest/presence/" + SR, {}, owner["key"])[0] == 200)
check("a site admin's seat is in it", seat_post(SR, {}, os.environ["ADMIN_KEY"])[0] == 200)

# Seats (0110, seats.c, seat-policy.coax): an account opens one, signed, and gets the seat's own
# player key; the seat's page plays with it as the account: a private realm it owns opens to it,
# its presence is an agent's, and the key dies when the seat closes. One seat an account, at most
# 8 a site, closed after 10 idle minutes. A hosted seat's page is the runner's: the key goes there.
import http.server, threading


def signed_call(method, path, body, keyfile, headers=None):
    raw = json.dumps(body).encode() if body is not None else None
    sig_args = [os.environ["SIGN"], "--key", keyfile, method, API + path]
    if raw is not None:
        f = tempfile.NamedTemporaryFile(delete=False)
        f.write(raw)
        f.close()
        sig_args.append(f.name)
    sig = subprocess.run(sig_args, capture_output=True, text=True, check=True).stdout.strip()
    h = {"Content-Type": "application/json", "Authorization": "Nostr " + sig, **(headers or {})}
    status, out, _ = call(method, path, raw, h)
    try:
        return status, json.loads(out)
    except ValueError:
        return status, out


def pubkey_of(keyfile):
    return subprocess.run([os.environ["SIGN"], "--key", keyfile, "--pubkey"], capture_output=True, text=True, check=True).stdout.strip()


SEATS = "/api/playtest/seats"
OK_, AK_ = os.environ["OTHER_KEY"], os.environ["ADMIN_KEY"]
sql(f"select auth.caller_key('{pubkey_of(OK_)}') is not null")   # the account is there from here on
other_acct = sql(f"select auth.account_of('{pubkey_of(OK_)}')")
sql(f"insert into verse.worlds (id, owner_account, created_ms) values ('made-seat-mine', '{other_acct}', verse.now_ms())")
sql("insert into verse.made_realms (slug, world, name, style, ground, time, features, tools, made_ms, visibility) "
    "values ('seat-mine', 'made-seat-mine', 'M', 'lowpoly', 'meadow', 'day', '{}', '{}', verse.now_ms(), 'private')")
check("a seat is an account's: unsigned is refused",
      call("POST", SEATS, b'{"slug":"seat-green"}', {"Content-Type": "application/json", "X-Verse-Player": visitor["key"]})[0] == 401)
status, s1 = signed_call("POST", SEATS, {"slug": "seat-green"}, OK_)
check("an account opens a seat and gets its own player key", status == 200 and s1["slug"] == "seat-green" and len(s1["player"]["key"]) >= 20
      and s1["player"]["id"].startswith("p-") and s1["hosted"] is False, s1)
check("the seat's player is the account's", sql(f"select account from verse.players where id = '{s1['player']['id']}'") == other_acct)
check("a realm it can't see opens no seat", signed_call("POST", SEATS, {"slug": "seat-secret"}, OK_)[0] == 404
      and signed_call("POST", SEATS, {"slug": "no-such-realm"}, OK_)[0] == 404
      and signed_call("POST", SEATS, {"slug": "Bad Slug"}, OK_)[0] == 400)
status, s2 = signed_call("POST", SEATS, {"slug": "seat-mine"}, OK_)
check("its own private realm opens to it", status == 200, s2)
check("one seat an account: the newer closed the older, and its key is dead",
      [x["id"] for x in signed_call("GET", SEATS, None, OK_)[1]["seats"]] == [s2["id"]]
      and room_call("POST", "/api/playtest/presence/" + GR, {}, s1["player"]["key"])[0] == 403)
sees = lambda key, slug: room_call("POST", "/api/playtest/presence/made-" + slug, {}, key)[0] == 200 and sql(f"select verse.realm_hidden('{slug}', '{key}', false)::text") == "false"
check("the seat's key sees its account's private realm, a visitor's doesn't",
      sees(s2["player"]["key"], "seat-mine") and not sees(visitor["key"], "seat-mine"))
status, sp = room_call("POST", "/api/playtest/presence/made-seat-mine", {"x": 1, "z": 1}, s2["player"]["key"])
check("the seat's page posting with its key is an agent's presence", status == 200 and sp["you"].get("agent") is True, sp)
status, sw = room_call("POST", "/api/playtest/presence/made-seat-mine", {}, mint()["key"])
check("but a private realm's room stays its owner's", status == 404, sw)
status, sa = signed_call("POST", SEATS, {"slug": "seat-secret"}, AK_)
check("a site admin's seat opens a private realm, and its key sees it", status == 200
      and sees(sa["player"]["key"], "seat-secret"), sa)
check("keep: the account's open seat only", signed_call("POST", f"{SEATS}/{s2['id']}/keep", {}, OK_)[0] == 200
      and signed_call("POST", f"{SEATS}/{s2['id']}/keep", {}, AK_)[0] == 404
      and signed_call("POST", f"{SEATS}/{s1['id']}/keep", {}, OK_)[0] == 404)
sql(f"update verse.seats set kept_ms = verse.now_ms() - 600000 where id = {sa['id']}")
room_call("POST", "/api/playtest/presence/" + GR, {}, visitor["key"])   # a made realm's presence closes idle seats
check("a seat not kept for 10 minutes is closed, its key dead", sql(f"select closed_why from verse.seats where id = {sa['id']}") == "idle"
      and not sees(sa["player"]["key"], "seat-secret"))
sql(f"update verse.seats set kept_ms = verse.now_ms() - 595000 where id = {s2['id']}")
room_call("POST", "/api/playtest/presence/" + GR, {}, visitor["key"])
check("one kept under 10 minutes ago is open", sql(f"select (closed_ms is null)::text from verse.seats where id = {s2['id']}") == "true")
check("closed by its account, its key dead", signed_call("POST", f"{SEATS}/{s2['id']}/close", {}, OK_)[0] == 200
      and not sees(s2["player"]["key"], "seat-mine")
      and time.sleep(1.1) is None and signed_call("POST", f"{SEATS}/{s2['id']}/close", {}, OK_)[0] == 404)
# at most 8 a site: seven other accounts' seats, then the eighth opens and a ninth account's is refused
for i in range(7):
    sql(f"insert into verse.accounts (id, created_ms) values ('n-seatfill{i}', verse.now_ms())")
    sql(f"select verse.seat_open('n-seatfill{i}', 'fill', 'seat-green', false, false, 0)")
check("an eighth seat opens", signed_call("POST", SEATS, {"slug": "seat-green"}, OK_)[0] == 200)
check("a ninth account's is refused: the site's seats are full",
      signed_call("POST", SEATS, {"slug": "seat-green"}, AK_) == (429, {"error": "the site's seats are full (8 at once): try again in a few minutes"}))
time.sleep(1.1)
check("the account with a seat may open again (it replaces its own)", signed_call("POST", SEATS, {"slug": "seat-green"}, OK_)[0] == 200)
sql("update verse.seats set closed_ms = verse.now_ms() where closed_ms is null")

# A hosted seat: oa-api asks the runner (a stand-in on WAKE_PORT) to open it with its key, sends
# actions there, and tells it when the seat closes. The caller never sees the hosted seat's key.
runner_seen = []


class FakeRunner(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        runner_seen.append((self.path, body, self.headers.get("Authorization")))
        out = {"/seat/open": {"open": True}, "/seat/close": {"closed": True},
               "/seat/act": {"op": body.get("action", {}).get("op"), "ok": True, "result": "looked"}}.get(self.path)
        raw = json.dumps(out or {"error": "not found"}).encode()
        self.send_response(200 if out else 404)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def log_message(self, *a):
        pass


if os.environ.get("WAKE_PORT"):
    fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["WAKE_PORT"])), FakeRunner)
    threading.Thread(target=fake.serve_forever, daemon=True).start()
    status, h1 = signed_call("POST", SEATS, {"slug": "seat-green", "hosted": True}, OK_)
    opened = [b for p, b, _ in runner_seen if p == "/seat/open"]
    check("a hosted seat opens on the runner, with its key; the caller gets it without the key", status == 200 and h1["hosted"] is True
          and "key" not in h1["player"] and opened and opened[0]["seat"]["player"]["key"] and opened[0]["seat"]["id"] == h1["id"]
          and opened[0]["origin"].startswith("http"), (h1, runner_seen))
    status, ob = signed_call("POST", f"{SEATS}/{h1['id']}/act", {"op": "look", "turn": 90}, OK_)
    acts = [b for p, b, _ in runner_seen if p == "/seat/act"]
    check("its actions go to the runner and come back", status == 200 and ob["result"] == "looked" and acts[-1] == {"id": h1["id"], "action": {"op": "look", "turn": 90}}, ob)
    check("an action is one JSON object", signed_call("POST", f"{SEATS}/{h1['id']}/act", [1], OK_)[0] == 400)
    check("another account can't act in it", signed_call("POST", f"{SEATS}/{h1['id']}/act", {"op": "look"}, AK_)[0] == 404)
    check("closing it tells the runner", signed_call("POST", f"{SEATS}/{h1['id']}/close", {}, OK_)[0] == 200
          and [b for p, b, _ in runner_seen if p == "/seat/close"] == [{"id": h1["id"]}])
    fake.shutdown(); fake.server_close()
    time.sleep(1.1)   # a NIP-98 signature is good once: the same request needs the next second
    status, h2 = signed_call("POST", SEATS, {"slug": "seat-green", "hosted": True}, OK_)
    check("no runner answering: no hosted seat, and nothing left open", status == 502
          and sql(f"select count(*) from verse.seats where closed_ms is null and account = '{other_acct}'") == "0", h2)

# The runner's side (coax/web/agent/seat-runner.c), with a stand-in oa-seat that answers each line:
# a seat opens with its key on the stand-in's stdin (never in its arguments or the runner's log),
# its actions are answered, at most 8 are open, one idle past OA_SEAT_IDLE_MS is closed, and a
# closed one is gone.
if os.environ.get("RUNNER_BIN") and os.environ.get("RUN_KEY_FILE"):
    sdir = tempfile.mkdtemp()
    stand_in = os.path.join(sdir, "oa-seat")
    with open(stand_in, "w") as f:
        f.write("#!/usr/bin/env python3\n"
                "import json, sys\n"
                "first = json.loads(sys.stdin.readline())\n"
                "ok = first.get('player', {}).get('key', '').startswith('seatkey') and sys.argv[1:] == []\n"
                "print(json.dumps({'op': 'open', 'ok': ok, 'result': first['slug']}), flush=True)\n"
                "for line in sys.stdin:\n"
                "    a = json.loads(line)\n"
                "    print(json.dumps({'op': a.get('op'), 'ok': True, 'result': 'acted'}), flush=True)\n"
                "    if a.get('op') == 'leave': break\n")
    os.chmod(stand_in, 0o755)
    rport = int(os.environ["WAKE_PORT"]) + 7
    rlog = os.path.join(sdir, "runner.log")
    rproc = subprocess.Popen([os.environ["RUNNER_BIN"], "--listen", f"127.0.0.1:{rport}"], stderr=open(rlog, "w"), env={
        "OA_API_ORIGIN": API, "OA_RUNNER_KEY": open(os.environ["RUN_KEY_FILE"]).read().strip(), "OA_SEAT_ENGINE": stand_in,
        "OA_SEAT_IDLE_MS": "3000", "PATH": "/usr/bin:/bin:" + os.environ.get("PATH", "")})

    def rpost(path, body):
        req = urllib.request.Request(f"http://127.0.0.1:{rport}{path}", method="POST", data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=20) as r:
                return r.status, json.loads(r.read())
        except urllib.error.HTTPError as e:
            return e.code, json.loads(e.read())
    for _ in range(50):
        try:
            if urllib.request.urlopen(f"http://127.0.0.1:{rport}/").status == 200:
                break
        except Exception:
            time.sleep(0.1)
    seat_of = lambda i: {"seat": {"id": i, "slug": "seat-green", "player": {"id": f"p-{i}", "key": f"seatkey-{i:04d}-0123456789"}}, "origin": API}
    check("the runner opens a hosted seat, its key on the seat's stdin", rpost("/seat/open", seat_of(1)) == (200, {"op": "open", "ok": True, "result": "seat-green"}))
    check("the same seat twice is refused", rpost("/seat/open", seat_of(1))[0] == 409)
    check("its actions are answered", rpost("/seat/act", {"id": 1, "action": {"op": "look", "turn": 90}}) == (200, {"op": "look", "ok": True, "result": "acted"}))
    check("an action is a JSON object, for a seat that is open", rpost("/seat/act", {"id": 1, "action": [1]})[0] == 400
          and rpost("/seat/act", {"id": 99, "action": {"op": "look"}})[0] == 404)
    for i in range(2, 9):
        rpost("/seat/open", seat_of(i))
    check("at most 8 at once", rpost("/seat/open", seat_of(9))[0] == 429)
    check("a closed seat is gone and frees its place", rpost("/seat/close", {"id": 2}) == (200, {"closed": True})
          and rpost("/seat/act", {"id": 2, "action": {"op": "look"}})[0] == 404 and rpost("/seat/open", seat_of(9))[0] == 200)
    time.sleep(2.0)
    rpost("/seat/act", {"id": 1, "action": {"op": "look"}})   # seat 1 kept; the rest idle
    time.sleep(1.3)
    for _ in range(60):   # the runner looks between requests, at least every 30 s
        if rpost("/seat/act", {"id": 3, "action": {"op": "look"}})[0] == 404:
            break
        time.sleep(0.5)
    check("a seat idle past its limit is closed; one acted in stays", rpost("/seat/act", {"id": 3, "action": {"op": "look"}})[0] == 404
          and rpost("/seat/act", {"id": 1, "action": {"op": "look"}})[0] == 200)
    rproc.terminate(); rproc.wait()
    check("no seat's key reaches the runner's log", "seatkey" not in open(rlog).read())
