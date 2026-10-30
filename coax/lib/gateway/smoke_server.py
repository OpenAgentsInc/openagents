# A local stand-in for a provider, for smoke.sh: POST / streams an Open
# Responses answer in three deltas a gap apart, then response.completed;
# POST /refuse answers 401; POST /whole answers one chat.completion
# after OpenRouter's keep-alive whitespace; POST /cut sends one delta and
# closes the stream. Each request checks the key header curl sent
# against GW_SMOKE_EXPECT and, while curl is still running, that no
# process's arguments hold the key. Prints its port, then serves forever.
import http.server, os, subprocess, sys, time

EXPECT = os.environ["GW_SMOKE_EXPECT"]
GAP = float(os.environ.get("GW_SMOKE_GAP", "0.4"))
problems = []

def leaked():
    out = subprocess.run(["ps", "-axww", "-o", "args="], capture_output=True, text=True).stdout
    return EXPECT in out

class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"
    def log_message(self, *a): pass
    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0") or 0)
        body = self.rfile.read(n) if n else b""
        auth = self.headers.get("Authorization", "")
        ok_key = auth == "Bearer " + EXPECT
        in_args = leaked()
        if self.path == "/whole":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b'\n   \n{"choices":[{"message":{"role":"assistant","content":"Four"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":1}}')
            return
        if self.path == "/cut":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            self.wfile.write(b'event: response.output_text.delta\ndata: {"type":"response.output_text.delta","delta":"Half"}\n\n')
            self.wfile.flush()
            return
        if self.path == "/refuse":
            self.send_response(401)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b'{"error":{"message":"bad key","type":"auth"}}')
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        def ev(name, data):
            self.wfile.write(f"event: {name}\ndata: {data}\n\n".encode()); self.wfile.flush()
        flags = ("key_ok" if ok_key else "key_wrong") + "," + ("key_in_args" if in_args else "args_clean") + "," + ("body_ok" if b'"model"' in body else "body_bad")
        for i, word in enumerate(["One\\n", "Two\\n", flags]):
            ev("response.output_text.delta", '{"type":"response.output_text.delta","delta":"%s"}' % word)
            time.sleep(GAP)
        ev("response.completed", '{"type":"response.completed","response":{"usage":{"input_tokens":7,"output_tokens":3}}}')

s = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
print(s.server_address[1], flush=True)
s.serve_forever()
