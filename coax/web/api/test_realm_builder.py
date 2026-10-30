# Build your own realm (realm-builder; realms.c, migrations/0048_realm_builder.sql): a prompt goes
# to a fake Gemini under the catalogue's schema and comes back as a board held to the catalogue;
# MAKE IT keeps only what is checked, gives the realm to the player, writes its page into the fake
# bucket (test_project_drops.py serves it); the owner's settings and the cards for what others
# propose. test.py runs this file with its helpers (call, check, sql).
import http.server
import threading
import time

builder_seen = []
builder_mode = {"status": 200, "text": "{}"}


class FakeBuilderModel(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        builder_seen.append(body)
        if builder_mode.get("stall", 0) > 0:   # a stalled try: answered only after the server has given up on it
            builder_mode["stall"] -= 1
            time.sleep(3)
        if builder_mode["status"] != 200:
            out = json.dumps({"error": {"code": builder_mode["status"]}}).encode()
        else:
            out = json.dumps({"candidates": [{"content": {"role": "model", "parts": [
                {"text": "planning", "thought": True}, {"text": builder_mode["text"]}]}}]}).encode()
        self.send_response(builder_mode["status"])
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


builder_fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["BUILDER_PORT"])), FakeBuilderModel)
threading.Thread(target=builder_fake.serve_forever, daemon=True).start()


def b_player():
    _, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
    return json.loads(raw)["key"]


def b_call(method, path, body=None, key=None, origin=None):
    h = {"Content-Type": "application/json"}
    if key:
        h["X-Verse-Player"] = key
    if origin:
        h["Origin"] = origin
    status, raw, headers = call(method, path, None if body is None else json.dumps(body).encode(), h)
    try:
        return status, json.loads(raw)
    except Exception:
        return status, raw.decode()



def realm_builder_checks():
    # the fake bucket test_project_drops.py served, served again for this file
    threading.Thread(target=gcs_server.serve_forever, daemon=True).start()
    ANSWER = {"name": "Moonlit Marsh", "style": "painterly", "ground": "forest", "time": "night", "features": [
        {"feature": "water", "on": True, "size": "large"},
        {"feature": "trees", "on": True, "variant": "dead", "count": 99},
        {"feature": "lanterns", "on": False, "colour": "plaid", "count": 6},
        {"feature": "flock", "on": True, "variant": "bat"},
        {"feature": "character", "on": True, "name": "Old Mott", "role": "keeps the bridge"},
        {"feature": "character", "on": True, "name": ""},
        {"feature": "dragon", "on": True},
        {"feature": "tool", "on": True, "tool": "talk"},
        {"feature": "tool", "on": True, "tool": "talk"},
        {"feature": "tool", "on": False, "tool": "build"},
        {"feature": "tool", "on": True, "tool": "teleport"},
        {"feature": "props", "on": True, "variant": "wagon"},
    ]}

    owner = b_player()
    builder_mode["text"] = json.dumps(ANSWER)
    st, plan = b_call("POST", "/api/playtest/realms/plan", {"prompt": "a dark marsh with bats and an old man by the bridge"}, owner)
    check("a prompt comes back as a board", st == 200 and plan["board"]["name"] == "Moonlit Marsh" and isinstance(plan["plan"], int), plan)
    board = plan.get("board", {})
    kinds = [f["feature"] + ":" + f.get("tool", f.get("variant", f.get("name", ""))) for f in board.get("features", [])]
    check("the board is held to the catalogue: an unknown feature, a nameless character, an unknown tool and a twice-named one are gone",
          kinds == ["water:", "trees:dead", "lanterns:", "flock:bat", "props:wagon", "character:Old Mott", "tool:talk", "tool:build"], kinds)
    f = {x["feature"]: x for x in board.get("features", [])}
    check("counts are held to their range and a word not the feature's is its default",
          f["trees"]["count"] == 20 and f["lanterns"]["colour"] == "gold" and f["flock"]["count"] == 7, f)
    check("each feature's words are written from its typed fields",
          f["water"]["label"] == "Large pond" and f["trees"]["label"] == "20 dead trees" and f["character"]["label"] == "Old Mott, keeps the bridge"
          and f["flock"]["label"] == "A flock of 7 bats", [x["label"] for x in board.get("features", [])])
    check("on and off come through", f["lanterns"]["on"] is False and f["water"]["on"] is True)
    seen = builder_seen[-1]
    schema = seen["generationConfig"]["responseSchema"]
    check("the model is asked under the catalogue's schema",
          schema["properties"]["ground"]["enum"][0] == "meadow" and "photoreal" not in schema["properties"]["style"]["enum"]
          and "character" in schema["properties"]["features"]["items"]["properties"]["feature"]["enum"], schema["properties"]["style"])
    check("the prompt is what was said", seen["contents"][0]["parts"][0]["text"].startswith("a dark marsh"))
    check("the model is asked with its thinking off, for a fast board (0060)", seen["generationConfig"]["thinkingConfig"] == {"thinkingBudget": 0}
          and plan.get("model") == "gemini-2.5-flash", (seen["generationConfig"].get("thinkingConfig"), plan.get("model")))

    # talking again revises the board: the current board goes with the new words
    import time
    time.sleep(2.1)
    st, again = b_call("POST", "/api/playtest/realms/plan", {"prompt": "no bats, and make it dusk", "board": board}, owner)
    check("talking again sends the current board", st == 200 and "The current board" in builder_seen[-1]["contents"][0]["parts"][0]["text"]
          and "no bats" in builder_seen[-1]["contents"][0]["parts"][0]["text"], builder_seen[-1]["contents"][0]["parts"][0]["text"][:200])
    check("a second prompt within two seconds waits", b_call("POST", "/api/playtest/realms/plan", {"prompt": "x"}, owner)[0] == 429)
    check("no prompt is a 400", b_call("POST", "/api/playtest/realms/plan", {"prompt": " "})[0] == 400)
    builder_mode["text"] = "not json"
    check("an answer that isn't json is a 503", b_call("POST", "/api/playtest/realms/plan", {"prompt": "a meadow"})[0] == 503)
    builder_mode["text"] = json.dumps({"name": "", "features": []})
    check("a board without a name is a 502", b_call("POST", "/api/playtest/realms/plan", {"prompt": "a meadow"})[0] == 502)
    builder_mode["status"] = 500
    check("the model's error is a 503", b_call("POST", "/api/playtest/realms/plan", {"prompt": "a meadow"})[0] == 503)
    builder_mode["status"] = 200
    check("a plan from another site's page is refused", b_call("POST", "/api/playtest/realms/plan", {"prompt": "a meadow"}, origin="https://evil.example")[0] == 403)

    # MAKE IT: what's off is never kept; the page is written into the site's bucket
    board["features"][0]["on"] = False          # no pond
    board["features"].append({"feature": "rocks", "on": True, "count": 4, "label": "a castle"})
    board["features"].append({"feature": "dragon", "on": True})
    check("making needs a player", b_call("POST", "/api/playtest/realms", {"board": board})[0] == 401)
    st, made = b_call("POST", "/api/playtest/realms", {"plan": plan["plan"], "board": board}, owner)
    check("MAKE IT makes the realm at /realms/<slug> and writes its page", st == 200 and made["slug"] == "moonlit-marsh" and made["path"] == "/realms/moonlit-marsh"
          and made["page"] is True, made)
    site = bucket["site"]
    check("its page, its bare path, its text and the list of made realms are in the bucket",
          all(n in site for n in ("realms/moonlit-marsh", "realms/moonlit-marsh/index.html", "realms/moonlit-marsh/index.md", "realms/made-slugs.txt"))
          and b"moonlit-marsh" in site["realms/made-slugs.txt"]["data"] and site["realms/moonlit-marsh"]["meta"]["cacheControl"] == "no-cache",
          sorted(n for n in site if n.startswith("realms/")))
    page = site.get("realms/moonlit-marsh", {}).get("data", b"").decode()
    check("the page opens the made-realm template for this realm", 'data-made="moonlit-marsh"' in page and 'fetch("/realms/built"' in page
          and "<title>Moonlit Marsh" in page, page[:300])
    md = site.get("realms/moonlit-marsh/index.md", {}).get("data", b"").decode()
    check("its text says what is there and what visitors may do", "- 20 dead trees" in md and "Large pond" not in md and "- Talk" in md and "Build" not in md, md)
    st, spec = b_call("GET", "/api/playtest/realms/moonlit-marsh", key=owner)
    lines = spec.split("\n") if isinstance(spec, str) else []
    check("the spec: only what was checked, the typed rocks (not the words sent), characters only those checked",
          "feature water large" not in lines and "feature trees dead 20" in lines and "feature lanterns gold 6" not in lines and "feature rocks 4" in lines
          and "character Old Mott|keeps the bridge" in lines and "feature props wagon" in lines and "style painterly" in lines and "owner 1" in lines
          and "tool talk 1" in lines and "tool build 0" in lines and "waiting 0" in lines, spec)
    st, theirs = b_call("GET", "/api/playtest/realms/moonlit-marsh")
    check("anyone may read it; only the owner is told they own it", st == 200 and "owner 0" in theirs and "waiting" not in theirs, theirs)
    check("an unknown realm is a 404", b_call("GET", "/api/playtest/realms/no-such-place")[0] == 404)
    check("the realm is a world its owner owns", sql("select owner_player is not null from verse.worlds where id = 'made-moonlit-marsh'") == "t")
    st, made2 = b_call("POST", "/api/playtest/realms", {"board": {"name": "Moonlit Marsh", "features": []}}, owner)
    check("a second realm of the same name gets its own address", st == 200 and made2["slug"] == "moonlit-marsh-2", made2)
    st, made3 = b_call("POST", "/api/playtest/realms", {"board": {"name": "Haunted House", "features": []}}, owner)
    check("a made realm never takes a built-in realm's address", st == 200 and made3["slug"] == "haunted-house-2", made3)
    st, empty = b_call("GET", "/api/playtest/realms/moonlit-marsh-2")
    check("a realm made with nothing checked is empty", st == 200 and "feature" not in empty and "character" not in empty, empty)
    for i in range(3):
        b_call("POST", "/api/playtest/realms", {"board": {"name": "Spare %d" % i, "features": []}}, owner)
    check("a player makes at most OA_BUILDER_PER_DAY realms a day", b_call("POST", "/api/playtest/realms", {"board": {"name": "One more", "features": []}}, owner)[0] == 429)

    # the owner decides what visitors get
    visitor = b_player()
    check("a visitor can't change the settings", b_call("POST", "/api/playtest/realms/moonlit-marsh/settings", {"tools": {"fly": True}}, visitor)[0] == 403)
    check("nor can someone without a key", b_call("POST", "/api/playtest/realms/moonlit-marsh/settings", {"tools": {"fly": True}})[0] == 403)
    check("a visitor can't build while build is off", b_call("POST", "/api/playtest/realms/moonlit-marsh/place", {"thing": "crate", "x": 1, "z": 2}, visitor)[0] == 403)
    st, s = b_call("POST", "/api/playtest/realms/moonlit-marsh/settings", {"tools": {"build": True, "fly": True, "talk": "yes", "teleport": True}}, owner)
    check("the owner switches tools on; a value that isn't true or false keeps what was", st == 200 and s["tools"]["build"] and s["tools"]["fly"]
          and s["tools"]["talk"] is True and "teleport" not in s["tools"], s)

    # what others build waits for the owner as a card
    st, p = b_call("POST", "/api/playtest/realms/moonlit-marsh/place", {"thing": "barrel", "x": 3, "z": -4, "yaw": 0.5}, visitor)
    check("a visitor's thing is proposed, not placed", st == 200 and isinstance(p.get("proposed"), int), p)
    check("a thing not in the list is refused", b_call("POST", "/api/playtest/realms/moonlit-marsh/place", {"thing": "dragon", "x": 0, "z": 0}, visitor)[0] == 400)
    check("so is one off the ground", b_call("POST", "/api/playtest/realms/moonlit-marsh/place", {"thing": "crate", "x": 99, "z": 0}, visitor)[0] == 400)
    st, spec = b_call("GET", "/api/playtest/realms/moonlit-marsh", key=owner)
    check("nothing changes until the owner accepts; the owner is told one waits", "placed" not in spec and "waiting 1" in spec, spec)
    st, cards = b_call("GET", "/api/playtest/realms/moonlit-marsh/proposals", key=owner)
    check("the owner's card: who, what, where, before and after", st == 200 and cards.startswith("P %d|Visitor " % p["proposed"]) and "|barrel|3|-4|0.5|0|1" in cards, cards)
    check("anyone else sees no cards", b_call("GET", "/api/playtest/realms/moonlit-marsh/proposals", key=visitor)[1] == "")
    check("a visitor can't accept", b_call("POST", "/api/playtest/realms/moonlit-marsh/proposals/%d/accept" % p["proposed"], {}, visitor)[0] == 403)
    st, d = b_call("POST", "/api/playtest/realms/moonlit-marsh/proposals/%d/accept" % p["proposed"], {}, owner)
    check("ACCEPT places it", st == 200 and d["state"] == "accepted" and "placed barrel 3 -4 0.5" in b_call("GET", "/api/playtest/realms/moonlit-marsh")[1], d)
    check("a card is decided once", b_call("POST", "/api/playtest/realms/moonlit-marsh/proposals/%d/decline" % p["proposed"], {}, owner)[0] == 409)
    st, p2 = b_call("POST", "/api/playtest/realms/moonlit-marsh/place", {"thing": "rock", "x": -2, "z": 1}, visitor)
    st, d = b_call("POST", "/api/playtest/realms/moonlit-marsh/proposals/%d/decline" % p2["proposed"], {}, owner)
    check("DECLINE changes nothing", st == 200 and d["state"] == "declined" and "placed rock" not in b_call("GET", "/api/playtest/realms/moonlit-marsh")[1], d)
    check("an unknown card is a 404", b_call("POST", "/api/playtest/realms/moonlit-marsh/proposals/999999/accept", {}, owner)[0] == 404)
    st, o = b_call("POST", "/api/playtest/realms/moonlit-marsh/place", {"thing": "lantern", "x": 0.5, "z": -3, "yaw": 0}, owner)
    check("the owner's own thing goes in at once", st == 200 and o.get("placed") is True and "placed lantern 0.5 -3" in b_call("GET", "/api/playtest/realms/moonlit-marsh")[1], o)
    st, _ = b_call("POST", "/api/playtest/realms/moonlit-marsh/settings", {"tools": {"build": False}}, owner)
    check("build switched off again stops visitors at once", b_call("POST", "/api/playtest/realms/moonlit-marsh/place", {"thing": "crate", "x": 1, "z": 1}, visitor)[0] == 403)
    check("a GET of the make route is a 405", b_call("GET", "/api/playtest/realms")[0] == 405)

    # the builder page reads lines, and sends back only which features are on
    time.sleep(2.1)
    lines_owner = b_player()
    builder_mode["text"] = json.dumps({"name": "Pine Hollow", "style": "lowpoly", "ground": "snow", "time": "dawn", "features": [
        {"feature": "trees", "on": True, "variant": "pine", "count": 12}, {"feature": "character", "on": True, "name": "Ilse", "role": "the guide"},
        {"feature": "tool", "on": True, "tool": "fly"}]})
    h = {"Content-Type": "application/json", "X-Verse-Player": lines_owner}
    st, raw, hd = call("POST", "/api/playtest/realms/plan?as=lines", json.dumps({"prompt": "snowy pines and a guide"}).encode(), h)
    text = raw.decode()
    plan_id = int(text.split("\n")[0].split(" ")[1]) if text.startswith("plan ") else 0
    check("?as=lines answers the board as lines", st == 200 and hd.get("Content-Type", "").startswith("text/plain") and plan_id > 0
          and "\nname Pine Hollow\nstyle lowpoly\nground snow\ntime dawn\n" in text and "f trees 1 12 pine trees\n" in text
          and "f character 1 Ilse, the guide\n" in text and "f tool 1 Visitors can fly\n" in text, text)
    st, raw, _ = call("POST", "/api/playtest/realms?as=lines", json.dumps({"plan": plan_id, "on": [True, False, True]}).encode(), h)
    made = raw.decode()
    check("MAKE IT with only the on switches: the plan's board, as the page switched it", st == 200 and made.startswith("slug pine-hollow\npath /realms/pine-hollow\npage 1"), made)
    st, spec = b_call("GET", "/api/playtest/realms/pine-hollow")
    check("a character switched off is not there", "feature trees pine 12" in spec and "character" not in spec and "tool fly 1" in spec, spec)
    st, raw, _ = call("POST", "/api/playtest/realms?as=lines", json.dumps({"plan": plan_id, "on": [True]}).encode(), {"Content-Type": "application/json", "X-Verse-Player": b_player()})
    check("someone else's plan makes nothing", st == 400 and raw.decode() == "no board", (st, raw))
    st, raw, _ = call("POST", "/api/playtest/realms/plan?as=lines", json.dumps({"prompt": ""}).encode(), h)
    check("an error as lines is its words alone", st == 400 and raw.decode() == "say what world you want", raw)

    # setup (0049): the visitor's answers are fixed; the model fills the rest
    time.sleep(2.1)
    setup_owner = b_player()
    h = {"Content-Type": "application/json", "X-Verse-Player": setup_owner}
    builder_mode["text"] = json.dumps({"name": "Dune Watch", "camera": "third", "style": "painterly", "ground": "forest", "time": "day",
                                       "features": [{"feature": "rocks", "on": True, "count": 5}]})
    st, raw, _ = call("POST", "/api/playtest/realms/plan?as=lines", json.dumps({"prompt": "a lookout over the dunes",
        "setup": {"camera": "first", "ground": "desert", "time": "night", "style": "photoreal", "colour": "red"}}).encode(), h)
    text = raw.decode()
    seen = builder_seen[-1]
    check("setup answers are fixed over the model's; one not in the catalogue is left to the model",
          st == 200 and "\nstyle painterly\nground desert\ntime night\ncamera first\n" in text, text)
    check("the model is told what is fixed and asked for a camera",
          seen["contents"][0]["parts"][0]["text"].startswith("Fixed in setup: ") and "photoreal" not in seen["contents"][0]["parts"][0]["text"]
          and seen["generationConfig"]["responseSchema"]["properties"]["camera"]["enum"] == ["third", "first", "top", "side"]
          and "camera" in seen["generationConfig"]["responseSchema"]["required"], seen["contents"][0]["parts"][0]["text"][:200])
    setup_plan = int(text.split("\n")[0].split(" ")[1]) if text.startswith("plan ") else 0
    st, raw, _ = call("POST", "/api/playtest/realms?as=lines", json.dumps({"plan": setup_plan, "on": [True], "setup": {"camera": "top", "time": "dusk"}}).encode(), h)
    check("an answer changed on the board goes with MAKE IT", st == 200 and raw.decode().startswith("slug dune-watch\n"), raw)
    st, spec = b_call("GET", "/api/playtest/realms/dune-watch")
    check("the made realm keeps its camera, and the setup as the board showed it",
          "camera top" in spec and "time dusk" in spec and "ground desert" in spec and "feature rocks 5" in spec, spec)
    md = bucket["site"].get("realms/dune-watch/index.md", {}).get("data", b"").decode()
    check("its text names the camera", "3D, top-down" in md, md[:200])
    st, old = b_call("GET", "/api/playtest/realms/moonlit-marsh")
    check("a realm made before cameras is third person", "camera third" in old, old)
    check("a realm named Plan never takes the builder's route", sql("select verse.builder_slug('Plan')") == "plan-2")

    # SAY IT for one question: the model picks one of its words, or none
    time.sleep(2.1)
    builder_mode["text"] = json.dumps({"value": "first"})
    st, raw, _ = call("POST", "/api/playtest/realms/pick?as=lines", json.dumps({"field": "camera", "words": "through my own eyes"}).encode(), h)
    seen = builder_seen[-1]
    check("a spoken answer is picked by the model under that question's words",
          st == 200 and raw.decode() == "field camera\nvalue first\n"
          and seen["generationConfig"]["responseSchema"]["properties"]["value"]["enum"] == ["third", "first", "top", "none"]
          and seen["contents"][0]["parts"][0]["text"] == "through my own eyes", (st, raw))
    time.sleep(2.1)
    builder_mode["text"] = json.dumps({"value": "fourth"})
    st, got = b_call("POST", "/api/playtest/realms/pick", {"field": "time", "words": "whenever"}, setup_owner)
    check("a word not the question's is none", st == 200 and got["value"] == "none" and got["field"] == "time", got)
    check("an unknown question is a 400", b_call("POST", "/api/playtest/realms/pick", {"field": "colour", "words": "red"})[0] == 400)
    check("no words is a 400", b_call("POST", "/api/playtest/realms/pick", {"field": "dims", "words": ""})[0] == 400)
    builder_mode["status"] = 500
    check("the model's error on a pick is a 503", b_call("POST", "/api/playtest/realms/pick", {"field": "dims", "words": "flat"})[0] == 503)
    builder_mode["status"] = 200
    check("SAY IT's pick is asked with its thinking off too", builder_seen[-1]["generationConfig"]["thinkingConfig"] == {"thinkingBudget": 0})

    # the slow tail (byo-plan-speed): a stalled try is cut off at OA_BUILDER_TIMEOUT_MS and tried once more
    time.sleep(2.1)
    builder_mode["text"] = json.dumps({"name": "Quick Fen", "features": [{"feature": "rocks", "on": True, "count": 4}]})
    builder_mode["stall"] = 1
    n0 = len(builder_seen)
    t0 = time.time()
    st, quick = b_call("POST", "/api/playtest/realms/plan", {"prompt": "a quick fen"}, b_player())
    took = time.time() - t0
    check("a stalled try is cut off and tried again: the board still comes, before the stalled one would have",
          st == 200 and quick["board"]["name"] == "Quick Fen" and len(builder_seen) - n0 == 2 and took < 2.9, (st, quick, len(builder_seen) - n0, took))
    time.sleep(2.1)
    builder_mode["stall"] = 2
    st, _ = b_call("POST", "/api/playtest/realms/plan", {"prompt": "a quick fen"}, b_player())
    check("two stalled tries are a 503, not a wait", st == 503, st)
    builder_mode["stall"] = 0
    time.sleep(3.2)
    gcs_server.shutdown()
    builder_fake.shutdown()


realm_builder_checks()
