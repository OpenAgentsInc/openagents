#!/usr/bin/env python3
"""A stand-in for a model served on this machine, for smoke.py's psionic
check: GET /v1/models lists one model; POST /v1/chat/completions answers
whole with Microcoder's next_action as text. The first request runs
`echo local-step`; once the state holds a command's output it finishes
with the reply "local-ok". Prints its port, then serves until killed.

Test-only harness; the program under test is Coax.
"""
import http.server, json, sys

def action(**kw):
    a = {"rationale": "stand-in", "commands": [], "view": [], "finished": False, "reply": "", "ask": "none"}
    a.update(kw)
    return json.dumps(a)

class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"
    def log_message(self, *a): pass
    def send(self, body):
        b = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)
    def do_GET(self):
        if self.path == "/v1/models":
            return self.send({"object": "list", "data": [{"id": "local-test", "object": "model"}]})
        self.send_error(404)
    def do_POST(self):
        req = self.rfile.read(int(self.headers.get("Content-Length", "0"))).decode()
        if self.path != "/v1/chat/completions" or self.headers.get("Authorization"):
            return self.send_error(400)
        done = "local-step" in req      # the state holds the command from the step before
        text = action(finished=True, reply="local-ok") if done else action(commands=["echo local-step"])
        self.send({"id": "x", "object": "chat.completion", "model": "local-test",
                   "choices": [{"index": 0, "message": {"role": "assistant", "content": text}, "finish_reason": "stop"}],
                   "usage": {"prompt_tokens": 10, "completion_tokens": 10, "total_tokens": 20}})

s = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
print(s.server_address[1], flush=True)
s.serve_forever()
