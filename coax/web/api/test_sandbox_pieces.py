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
        if piece_mode["status"] != 200:
            out = json.dumps({"error": {"code": piece_mode["status"]}}).encode()
        else:
            out = json.dumps({"candidates": [{"content": {"role": "model", "parts": [
                {"text": "thinking about it", "thought": True}, {"text": piece_mode["text"]}]}, "finishReason": "STOP"}]}).encode()
        self.send_response(piece_mode["status"])
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
