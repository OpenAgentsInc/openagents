# The Sandbox's set pieces (rl-sandbox-pieces, migrations/0036_sandbox_pieces.sql):
# a sentence goes to a fake Gemini that test.sh points the service at, under
# the catalogue's response schema, and its answer is held to the catalogue.
# test.py runs this file.
import http.server
import re
import threading

piece_seen = []
piece_mode = {"status": 200, "text": "{}"}


class FakePieceModel(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        piece_seen.append({"auth": self.headers.get("Authorization"), "body": body})
        once = piece_mode.pop("once", None)   # a first try that fails ("503") or stalls ("stall") before the real answer
        if once == "stall":
            time.sleep(2.5)
        if once == "503" or piece_mode["status"] != 200:
            out = json.dumps({"error": {"code": 503 if once == "503" else piece_mode["status"]}}).encode()
        else:
            out = json.dumps({"candidates": [{"content": {"role": "model", "parts": [
                {"text": "thinking about it", "thought": True}, {"text": piece_mode["text"]}]}, "finishReason": "STOP"}]}).encode()
        self.send_response(503 if once == "503" else piece_mode["status"])
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


piece_fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["PIECE_PORT"])), FakePieceModel)
threading.Thread(target=piece_fake.serve_forever, daemon=True).start()


def make(sentence, answer=None, key=None):
    if answer is not None:
        piece_mode["text"] = answer if isinstance(answer, str) else json.dumps(answer)
    headers = {"Content-Type": "application/json"}
    if key:
        headers["X-Verse-Player"] = key
    status, raw, _ = call("POST", "/api/playtest/sandbox/piece", json.dumps({"sentence": sentence}).encode(), headers)
    return status, json.loads(raw)


status, a = make("a flock of birds follows me", {"piece": "flock", "count": 9, "kind": "crow"})
check("a sentence makes the piece the model picked, with its fields", status == 200 and a == {"piece": "flock", "count": 9, "kind": "crow", "model": "gemini-3.8-flash"}, a)
sent = piece_seen[-1]["body"]
check("the service's token goes with it", piece_seen[-1]["auth"] == "Bearer test-token")
check("the sentence is the model's only words from the visitor", sent["contents"] == [{"role": "user", "parts": [{"text": "a flock of birds follows me"}]}], sent["contents"])
schema = sent["generationConfig"]["responseSchema"]
check("the model answers under the catalogue's schema: one piece of four, or none",
      sent["generationConfig"]["responseMimeType"] == "application/json" and schema["properties"]["piece"]["enum"] == ["flock", "lanterns", "grove", "pond", "none"]
      and schema["required"] == ["piece"] and sorted(schema["properties"]["kind"]["enum"]) == ["bat", "crow", "dead", "mixed", "pine", "round"], schema)
system = sent["systemInstruction"]["parts"][0]["text"]
check("the prompt names every piece and its bounds",
      "- flock: a flock of birds that flies round whoever is walking and follows them. count 3 to 12 (usually 7). kind: crow, bat" in system
      and "- lanterns: a ring of glowing lanterns. count 4 to 16 (usually 8). colour: orange, gold, red, green, blue, violet, white. size: small, medium or large" in system
      and "- pond: a pond ringed with stones and bushes. size: small, medium or large" in system and 'answer piece "none"' in system, system)

# The page's catalogue (verse/setpiece.coax) and the server's are the same.
coax = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../verse/setpiece.coax")).read()
def coax_list(name):
    return re.findall(r'"([^"]*)"', re.search(name + r' :: \[[A-Z_]+\] string\.\[([^\]]*)\]', coax).group(1))
def coax_ints(name):
    return [int(x) for x in re.search(name + r' :: \[[A-Z_]+\] int\.\[([^\]]*)\]', coax).group(1).split(",")]
cat = json.loads(sql("select verse.piece_catalogue()::text"))
names = coax_list("PIECE_NAMES")
kinds, first, many = coax_list("KIND_NAMES"), coax_ints("PIECE_KIND_FIRST"), coax_ints("PIECE_KIND_N")
lo, hi, usual = coax_ints("PIECE_COUNT_MIN"), coax_ints("PIECE_COUNT_MAX"), coax_ints("PIECE_COUNT_DEFAULT")
same = names == json.loads(sql("select to_jsonb(verse.piece_order())::text")) and sorted(names) == sorted(cat)
for i, n in enumerate(names):
    same = same and cat[n].get("kind", []) == kinds[first[i]:first[i] + many[i]]
    same = same and cat[n].get("count", [0, 0, 0]) == [lo[i], hi[i], usual[i]]
    same = same and ("colour" in cat[n]) == (n == "lanterns") and cat[n].get("colour", coax_list("COLOUR_NAMES")) == coax_list("COLOUR_NAMES")
    same = same and ("size" in cat[n]) == (n != "flock")
check("the server's catalogue is the page's (verse/setpiece.coax)", same, cat)

status, a = make("forty blue bats", {"piece": "flock", "count": 40, "kind": "bat", "colour": "blue", "size": "large"})
check("a count is held to the piece's range, and fields it doesn't have are left out", status == 200 and a == {"piece": "flock", "count": 12, "kind": "bat", "model": "gemini-3.8-flash"}, a)
status, a = make("a grove of palms", {"piece": "grove", "kind": "palm", "count": 2.6})
check("a kind not the piece's is its default; a count is rounded and held", status == 200 and a["kind"] == "round" and a["count"] == 3 and a["size"] == "medium", a)
status, a = make("lanterns please", {"piece": "lanterns"})
check("missing fields are the piece's defaults", status == 200 and a == {"piece": "lanterns", "count": 8, "colour": "orange", "size": "medium", "model": "gemini-3.8-flash"}, a)
status, a = make("a little pond", {"piece": "pond", "size": "small", "count": 5})
check("a pond has a size only", status == 200 and a == {"piece": "pond", "size": "small", "model": "gemini-3.8-flash"}, a)
status, a = make("build me a castle", {"piece": "none"})
check("nothing in the catalogue fits: a 422", status == 422 and a["error"] == "none fit", a)
status, a = make("a dragon", {"piece": "dragon", "count": 1})
check("an answer that isn't a piece is refused", status == 502 and a["error"] == "not a piece", a)
status, a = make("anything", "not json at all")
check("an answer that isn't JSON is no answer", status == 503, a)
check("every sentence and what was picked is kept",
      sql("select sentence||'|'||coalesce(piece::text,'')||'|'||coalesce(error,'') from verse.sandbox_pieces order by id desc limit 3")
      == 'anything||no answer\na dragon||not a piece\nbuild me a castle||none fit', sql("select sentence, piece, error from verse.sandbox_pieces order by id desc limit 3"))
piece_mode["status"] = 500
status, a = make("a pond")
check("a model error is a 503", status == 503 and a["error"] == "model error 500", a)
piece_mode["status"] = 200
check("no sentence is a 400", make("   ")[0] == 400)
check("a GET is a 405", call("GET", "/api/playtest/sandbox/piece")[0] == 405)
status, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
builder = json.loads(raw).get("key", "")
first_try = make("a pond", {"piece": "pond"}, key=builder)
second_try = make("another pond", key=builder)
check("a player's set pieces come one at a time", first_try[0] == 200 and second_try[0] == 429 and make("a third pond")[0] == 200, (first_try, second_try))
check("a player's piece is kept as theirs", sql("select count(*) from verse.sandbox_pieces s join verse.players p on p.id = s.player") == "1")

# ---- an admin's spoken edit to a realm (eg-voice-edit, migrations/0045_realm_voice_edit.sql): the same
# fake model, an admin only; the edit the page then publishes is an admin's write (test_realm_edit.py).
admin_hex = os.environ["OA_ADMIN_PUBKEYS"].strip()
for tok, pk in (("9" * 64, admin_hex), ("8" * 64, "7" * 64)):
    sql(f"insert into signin.sessions (token_sha256, pubkey, created_ms, expires_ms) values (encode(sha256('{tok}'), 'hex'), '{pk}', 0, 9999999999999) on conflict do nothing")


def voice_edit(sentence, answer=None, cookie="9" * 64, realm="everglade", origin=API, wait=True):
    if answer is not None:
        piece_mode["text"] = answer if isinstance(answer, str) else json.dumps(answer)
    h = {"Content-Type": "application/json"}
    if cookie: h["Cookie"] = "oa_session=" + cookie
    if origin: h["Origin"] = origin
    if wait: time.sleep(0.05)
    s, raw, _ = call("POST", f"/api/playtest/realm/{realm}/edit", json.dumps({"sentence": sentence}).encode(), h)
    return s, json.loads(raw)


import time
M = {"model": "gemini-2.5-flash"}   # its thinking off (migrations/0054_voice_edit_fast.sql)
def said(**kw):   # a whole answer as the model gives it: every field, "not said" values for the rest
    a = {"edit": "none", "item": "something", "name": "", "count": 1, "arrange": "front", "onto": "ground", "colour": "unsaid", "size": 1,
         "direction": "here", "metres": 0, "turn": 0, "all": False, "who": "nobody", "building": "unnamed"}
    a.update(kw)
    return a
s, a = voice_edit("put two barrels here", said(edit="place", item="barrel", count=2))
check("voice edit: an admin's sentence becomes the edit the model picked, without its not-said fields", s == 200 and a == {"edit": "place", "item": "barrel", "count": 2, **M}, (s, a))
gc = piece_seen[-1]["body"]["generationConfig"]
check("voice edit: the planner is asked with its thinking off, for a fast answer", gc.get("thinkingConfig") == {"thinkingBudget": 0}, gc)
s, a = voice_edit("put a crate here", said(edit="place", item="crate"), wait=False)
check("voice edit: the next sentence goes the moment the last is answered", s == 200 and a.get("item") == "crate", (s, a))
sql("insert into verse.voice_edits (ms, realm, pubkey, sentence) values (verse.now_ms(), 'everglade', '" + admin_hex + "', 'still with the model')")
s, a = voice_edit("put a crate here", said(edit="place", item="crate"))
check("voice edit: one at a time: not while another is with the model", s == 429, (s, a))
sql("update verse.voice_edits set ms_taken = 1 where sentence = 'still with the model'")
piece_mode["once"] = "503"
n = len(piece_seen)
s, a = voice_edit("put two barrels here", said(edit="place", item="barrel", count=2))
check("voice edit: a failed try is tried once more", s == 200 and a.get("item") == "barrel" and len(piece_seen) == n + 2, (s, a, len(piece_seen) - n))
piece_mode["once"] = "stall"
n = len(piece_seen)
s, a = voice_edit("put two barrels here", said(edit="place", item="barrel", count=2))
taken = sql("select ms_taken from verse.voice_edits order by id desc limit 1")
check("voice edit: a stalled try is cut off and tried again, and the time both took is kept",
      s == 200 and a.get("item") == "barrel" and len(piece_seen) == n + 2 and 1500 <= int(taken) < 2500, (s, a, len(piece_seen) - n, taken))
sent = piece_seen[-1]["body"]
check("voice edit: the words are the model's only input from the admin", sent["contents"] == [{"role": "user", "parts": [{"text": "put two barrels here"}]}])
schema = sent["generationConfig"]["responseSchema"]
props = schema["properties"]
check("voice edit: under the realm's catalogue as enums",
      props["edit"]["enum"] == ["place", "remove", "move", "resize", "rotate", "restore", "list", "undo", "none"]
      and len(props["item"]["enum"]) == 38 and props["item"]["enum"][:5] == ["crate", "barrel", "bench", "fence", "stall"]
      and props["item"]["enum"][34:] == ["person", "cat", "building", "something"]
      and props["onto"]["enum"] == ["ground", "stall", "table", "crate", "barrel", "bench", "cart", "stool"]
      and props["arrange"]["enum"] == ["front", "row", "ring", "scatter", "fill"] and props["colour"]["enum"][-1] == "unsaid"
      and props["direction"]["enum"][0] == "here" and props["building"]["enum"][:2] == ["unnamed", "inn"] and "temple" in props["building"]["enum"]
      and props["all"]["type"] == "boolean" and props["name"]["type"] == "string" and "Old Tam" in props["who"]["enum"], schema)
here = os.path.dirname(os.path.abspath(__file__))
edits_coax = open(os.path.join(here, "../../../verse/everglade/edits.coax")).read()
named = re.search(r'EG_EDIT_NAME :: \[(\d+)\] string\.\[([^\]]*)\]', edits_coax)
if named:   # the page's names, in the contract's order
    coax_items = re.findall(r'"([^"]*)"', named.group(2))
    check("voice edit: the server's things are the page's (verse/everglade/edits.coax EG_EDIT_NAME)", coax_items == props["item"]["enum"][:34], coax_items)
else:       # until the page's names land: its first five labels
    coax_items = re.findall(r'"([^"]*)"', re.search(r'EG_EDIT_LABEL :: \[5\] string\.\[([^\]]*)\]', edits_coax).group(1))
    check("voice edit: the server's things are the page's (verse/everglade/edits.coax)", [x.lower() for x in coax_items] == props["item"]["enum"][:5], coax_items)
coax_people = re.findall(r'"([^"]*)"', re.search(r'EG_PERSON :: \[22\] string\.\[([^\]]*)\]', open(os.path.join(here, "../../../verse/everglade/people.coax")).read()).group(1))
check("voice edit: the server's people are the page's (verse/everglade/people.coax), after nobody", props["who"]["enum"] == ["nobody"] + coax_people, coax_people)
order = ["edit", "item", "name", "count", "arrange", "onto", "colour", "size", "direction", "metres", "turn", "all", "who", "building"]
check("voice edit: every field is answered, edit and item first, then name", schema["required"] == order and schema["propertyOrdering"] == order, schema)
prompt = sent["systemInstruction"]["parts"][0]["text"]
check("voice edit: the prompt lists every thing with its size, and shows the logged sentences",
      all(i in prompt for i in props["item"]["enum"][:34]) and "stall 2.9x1.5x2.6" in prompt and "tower 4x4x10" in prompt
      and "put pumpkins on the stall => place, item pumpkin, count 3, onto stall" in prompt and "put a building in front of me => place, item house" in prompt
      and "what else can I put down? => list" in prompt and "this barrel is bigger => resize, item barrel, size 1.5" in prompt
      and "movies crates" in prompt and "place 12 benches all around => place, item bench, count 12, arrange ring" in prompt
      and "delete this temple building => remove, item building, building temple" in prompt and "four big crates in front => place, item crate, count 4, size 1.5" in prompt
      and "fill the field with purple flowers => place, item flowers, colour purple, arrange fill" in prompt
      and "place a dragon => place, item something, name dragon" in prompt and "remove Bob => remove, item person, name Bob" in prompt
      and "remove these crates => remove, item crate, all true" in prompt and "what time is it => none" in prompt, prompt)

def held(sentence, answer, want, what):
    s, a = voice_edit(sentence, said(**answer))
    check("voice edit: " + what, s == 200 and a == {**want, **M}, (s, a))
held("a hundred crates", dict(edit="place", item="crate", count=100, turn=45, all=True, who="Marta"), {"edit": "place", "item": "crate", "count": 40}, "a count is held to 1..40, other edits' fields dropped")
held("some crates", dict(edit="place", item="crate", count=0), {"edit": "place", "item": "crate", "count": 1}, "a count under one is one")
held("six pumpkins on the stall", dict(edit="place", item="pumpkin", count=6, onto="stall"), {"edit": "place", "item": "pumpkin", "count": 6, "onto": "stall"}, "place onto a thing with a top")
held("fill the field with purple flowers", dict(edit="place", item="flowers", arrange="fill", colour="purple"), {"edit": "place", "item": "flowers", "count": 1, "arrange": "fill", "colour": "purple"}, "fill, with a colour")
held("twelve benches around me", dict(edit="place", item="bench", count=12, arrange="ring"), {"edit": "place", "item": "bench", "count": 12, "arrange": "ring"}, "a ring")
held("four enormous crates", dict(edit="place", item="crate", count=4, size=9), {"edit": "place", "item": "crate", "count": 4, "size": 4}, "a size is held to 0.25..4")
held("a tiny crate", dict(edit="place", item="crate", size=0.1, onto="table", colour="green"), {"edit": "place", "item": "crate", "count": 1, "size": 0.25, "onto": "table"}, "a colour not of the list is dropped")
held("a crate on the pot", dict(edit="place", item="crate", onto="pot", arrange="sideways"), {"edit": "place", "item": "crate", "count": 1}, "onto only a thing with a top, arrange only of the list")
held("place a dragon", dict(edit="place", item="something", name="dragon"), {"edit": "missing", "name": "dragon"}, "a place of a thing not in the list is missing, by its name")
held("put down a cat", dict(edit="place", item="cat", name="cat"), {"edit": "missing", "name": "cat"}, "people and cats aren't placed")
s, a = voice_edit("place a thing", said(edit="place", item="something"))
check("voice edit: a place of nothing it knows and no name is not an edit", s == 502, (s, a))
s, a = voice_edit("place a dragon", {"edit": "place", "item": "dragon"})
check("voice edit: an item not in the catalogue, no name: not an edit", s == 502, (s, a))
held("undo that", dict(edit="undo", item="crate", count=3), {"edit": "undo"}, "undo has no fields")
held("what else can I put down?", dict(edit="list", item="crate"), {"edit": "list"}, "list has no fields")
held("remove these crates", dict(edit="remove", item="crate", all=True, count=4), {"edit": "remove", "item": "crate", "all": True}, "remove these crates is every crate near")
held("remove these crates", dict(edit="remove", item="crate", who="Marta"), {"edit": "remove", "item": "crate"}, "a name never rides along with a placed thing's kind")
held("remove these cats", dict(edit="remove", item="cat", all=True, who="Soot"), {"edit": "remove", "item": "cat", "all": True}, "nor with many")
held("remove that", dict(edit="remove"), {"edit": "remove"}, "remove that is the nearest")
held("clear this", dict(edit="remove", all=True), {"edit": "remove", "all": True}, "clear this")
held("remove that", dict(edit="remove", who="Nobody"), {"edit": "remove"}, "a name not of the city is dropped")
held("remove Old Tam", dict(edit="remove", who="Old Tam", item="person"), {"edit": "remove", "item": "person", "who": "Old Tam"}, "remove a person by name")
held("delete this temple building", dict(edit="remove", item="building", building="temple"), {"edit": "remove", "item": "building", "building": "temple"}, "remove a building of the city")
held("remove the inn", dict(edit="remove", building="inn"), {"edit": "remove", "item": "building", "building": "inn"}, "a building named is a building")
held("remove that house", dict(edit="remove", item="house", building="house"), {"edit": "remove", "item": "house"}, "a placed house is not the city's")
held("remove Bob", dict(edit="remove", item="person", name="Bob"), {"edit": "missing", "name": "Bob"}, "a person not of the city is missing")
held("remove the crates and Bob", dict(edit="remove", item="crate", all=True, name="Bob"), {"edit": "remove", "item": "crate", "all": True}, "a known thing named wins over a missing one")
held("movies crates", dict(edit="move", item="crate", all=True, direction="here", metres=0, size=2), {"edit": "move", "item": "crate", "all": True, "direction": "here"}, "move, here")
held("move it left forty metres", dict(edit="move", direction="left", metres=40), {"edit": "move", "direction": "left", "metres": 30}, "metres are held to 0.5..30")
held("nudge it", dict(edit="move", direction="sideways", metres=0.1), {"edit": "move", "direction": "here", "metres": 0.5}, "a direction not of the list is here")
held("move Marta", dict(edit="move", item="person", who="Marta"), {"edit": "move", "direction": "here"}, "move takes placed things only")
held("this barrel is bigger", dict(edit="resize", item="barrel", size=1.5, count=2), {"edit": "resize", "item": "barrel", "size": 1.5}, "resize")
held("make it huge", dict(edit="resize", size=10), {"edit": "resize", "size": 4}, "a resize is held to 0.25..4")
held("resize it", dict(edit="resize", size=1), {"edit": "resize", "size": 1.5}, "a resize of no factor is bigger")
held("turn it around", dict(edit="rotate", turn=180), {"edit": "rotate", "turn": 180}, "rotate")
held("spin these benches", dict(edit="rotate", item="bench", all=True, turn=720), {"edit": "rotate", "item": "bench", "all": True, "turn": 360}, "a turn is held to -360..360")
held("turn it", dict(edit="rotate"), {"edit": "rotate", "turn": 90}, "a turn of no amount is 90")
held("bring back the temple", dict(edit="restore", building="temple", all=True, item="crate"), {"edit": "restore", "building": "temple"}, "restore a building")
held("bring Marta back", dict(edit="restore", who="Marta"), {"edit": "restore", "who": "Marta"}, "restore a person")
held("bring back Bob", dict(edit="restore", name="Bob"), {"edit": "missing", "name": "Bob"}, "restore of someone not of the city is missing")
s, a = voice_edit("summon a lich", {"edit": "missing", "name": "lich"})
check("voice edit: the model can't answer missing itself", s == 502, (s, a))
s, a = voice_edit("what time is it", said(edit="none"))
check("voice edit: nothing fits", s == 422, (s, a))
s, a = voice_edit("put a crate here", said(edit="place", item="crate"), cookie="8" * 64)
check("voice edit: someone logged in who isn't an admin is refused", s == 403, (s, a))
s, a = voice_edit("put a crate here", said(edit="place", item="crate"), cookie=None)
check("voice edit: signed out is refused", s == 401, (s, a))
s, a = voice_edit("put a crate here", said(edit="place", item="crate"), origin="https://evil.example")
check("voice edit: another site's page is refused", s == 403, (s, a))
s, a = voice_edit("put a crate here", said(edit="place", item="crate"), realm="ice-cream-man")
check("voice edit: a realm without voice edits says so", s == 400, (s, a))
check("voice edit: every sentence is kept with the admin who said it",
      sql("select count(*) from verse.voice_edits where pubkey = '" + admin_hex + "'") == "46" and sql("select count(*) from verse.voice_edits where pubkey <> '" + admin_hex + "'") == "0",
      sql("select count(*) from verse.voice_edits"))
