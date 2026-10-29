# Character chat (pt-chat-endpoint): the demo's prompt and memory, asked of a
# fake Gemini that test.sh points the service at. test.py runs this file.
import http.server
import threading

seen = []
mode = {"status": 200}


class FakeModel(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        seen.append({"auth": self.headers.get("Authorization"), "body": body})
        if mode["status"] != 200:
            out = json.dumps({"error": {"code": mode["status"]}}).encode()
        else:
            out = json.dumps({"candidates": [{"content": {"role": "model", "parts": [
                {"text": "Mortimer, at your service.", "thoughtSignature": "x"}, {"text": "The house keeps its own counsel."}]},
                "finishReason": "STOP"}]}).encode()
        self.send_response(mode["status"])
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["NPC_PORT"])), FakeModel)
threading.Thread(target=fake.serve_forever, daemon=True).start()


def ask(body):
    status, raw, _ = call("POST", "/api/playtest/npc/ask", json.dumps(body).encode(), {"Content-Type": "application/json"})
    return status, json.loads(raw)


q = {"npc": {"name": "Mortimer", "role": "the butler", "greet": "Good evening.", "knowledge": "The cellar key is under the rug."},
     "world": "Haunted House",
     "history": [{"from": "npc", "text": "Good evening."}, {"from": "player", "text": "Hello"}, {"from": "player", "text": "Anyone home?"},
                 {"from": "npc", "text": "Only me."}],
     "question": "Where is the cellar key?"}
status, a = ask(q)
check("a question is answered in character", status == 200 and a == {"text": "Mortimer, at your service. The house keeps its own counsel.", "model": "gemini-3.8-flash"}, a)
sent = seen[-1]["body"]
system = sent["systemInstruction"]["parts"][0]["text"]
check("the service's token goes with it", seen[-1]["auth"] == "Bearer test-token")
check("the demo's prompt: who, where, role, greeting, the rules and the controls",
      system.startswith("You are Mortimer, a character in Haunted House, a small 3D world in the Verse") and "Your role: the butler. " in system
      and "You greet people with: Good evening. " in system and "one to three short, plain sentences" in system and "B opens edit mode" in system, system)
check("the web's controls: ASK, TEACH, HINT and BYE in a conversation, not the old demo's",
      "ASK (or T) to type a question" in system and "TEACH to type something" in system and "HINT (or E)" in system
      and "BYE (or Esc)" in system and "X is delete mode" not in system and "teach <name>" not in system, system)
check("what players taught it", system.endswith("What players have taught you (true in this world):\nThe cellar key is under the rug."))
check("the memory: a speaker's lines run together, the model's first line dropped, the question last",
      sent["contents"] == [{"role": "user", "parts": [{"text": "Hello\nAnyone home?"}]}, {"role": "model", "parts": [{"text": "Only me."}]},
                           {"role": "user", "parts": [{"text": "Where is the cellar key?"}]}], sent["contents"])
check("every question and answer is kept",
      sql("select npc||'|'||world||'|'||question||'|'||answer from verse.npc_log order by id desc limit 1")
      == "Mortimer|Haunted House|Where is the cellar key?|Mortimer, at your service. The house keeps its own counsel.")
check("no question is a 400", ask({"npc": {"name": "Agatha"}})[0] == 400)
check("a GET is a 405", call("GET", "/api/playtest/npc/ask")[0] == 405)
mode["status"] = 500
status, a = ask({"question": "hello?"})
check("a model error is a 503, so the page says its own lines", status == 503 and a["error"] == "model error 500", a)
check("an unnamed character in an unnamed world", seen[-1]["body"]["systemInstruction"]["parts"][0]["text"].startswith("You are Someone, a character in this world"))
mode["status"] = 200
for _ in range(3):
    ask({"question": "again"})
status, a = ask({"question": "one too many"})
check("the day's answers are capped across the server", status == 503 and "used up" in a["error"], a)
status, raw, _ = call("GET", "/api/playtest/health")
check("the service answered other requests meanwhile", status == 200)

# Characters who remember you (eg-people, migrations/0031_npc_memory.sql): with a
# player key, what that player told a character on an earlier visit is in its prompt.
sql("delete from verse.npc_day")
status, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
wren = json.loads(raw)


def ask_as(key, visit, question, npc="Innkeeper"):
    body = {"npc": {"name": npc, "role": "the innkeeper"}, "world": "Everglade", "visit": visit, "question": question,
            "controls": "W A S D walk, E talks to whoever is near."}
    headers = {"Content-Type": "application/json"}
    if key:
        headers["X-Verse-Player"] = key
    status, raw, _ = call("POST", "/api/playtest/npc/ask", json.dumps(body).encode(), headers)
    return status, seen[-1]["body"]["systemInstruction"]["parts"][0]["text"]


status, system = ask_as(wren["key"], "visit-1", "I'm Wren, and blackberry pie is my favourite.")
check("a world names its own controls: no Haunted House edit mode in Everglade",
      status == 200 and "E talks to whoever is near." in system and "B opens edit mode" not in system, system)
check("a first meeting has nothing to remember", "talked with you before" not in system, system)
check("the log keeps who asked and on which visit",
      sql("select player||'|'||visit from verse.npc_log order by id desc limit 1") == wren["id"] + "|visit-1")
status, system = ask_as(wren["key"], "visit-1", "What's good tonight?")
check("on the same visit the conversation is the page's own history, not memory", "talked with you before" not in system, system)
status, system = ask_as(wren["key"], "visit-2", "Remember me?")
check("on a later visit the character is told what the player said before, oldest first",
      "This player has talked with you before, on earlier visits." in system
      and system.index("They said: I'm Wren, and blackberry pie is my favourite.") < system.index("They said: What's good tonight?")
      and "You answered: Mortimer, at your service." in system, system)
status, system = ask_as(None, "visit-3", "Remember me?")
check("without a player key nobody is remembered", status == 200 and "talked with you before" not in system, system)
status, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
other = json.loads(raw)
status, system = ask_as(other["key"], "visit-4", "Remember me?")
check("one player's words are never told to another", "Wren" not in system and "talked with you before" not in system, system)
status, system = ask_as(wren["key"], "visit-5", "Hello", npc="Smith")
check("each character remembers only what was said to it", "talked with you before" not in system, system)
fake.shutdown()
