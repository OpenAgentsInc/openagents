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
fake.shutdown()
