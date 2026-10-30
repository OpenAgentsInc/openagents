# /api/v1/inference (gw-ours): a signed Lab request reaches a fake Vertex that
# test.sh points the service at; the answer comes back as Vertex gave it and
# the call is metered by account. test.py runs this file.
import http.server
import threading

infer_seen = []
infer_mode = {"status": 200}


class FakeVertex(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        infer_seen.append({"auth": self.headers.get("Authorization"), "body": body})
        if infer_mode["status"] != 200:
            out, kind = json.dumps({"error": {"code": infer_mode["status"], "message": "Resource exhausted"}}).encode(), "application/json"
        elif body.get("stream"):
            chunks = [{"choices": [{"index": 0, "delta": {"role": "assistant", "content": "Par"}}]},
                      {"choices": [{"index": 0, "delta": {"content": "is."}, "finish_reason": "stop"}]},
                      {"choices": [], "usage": {"prompt_tokens": 2000, "completion_tokens": 400, "total_tokens": 2400}}]
            out, kind = ("".join("data: " + json.dumps(c) + "\n\n" for c in chunks) + "data: [DONE]\n\n").encode(), "text/event-stream"
        else:
            out, kind = json.dumps({"choices": [{"index": 0, "message": {"role": "assistant", "content": "Paris."}, "finish_reason": "stop"}],
                                    "usage": {"prompt_tokens": 1000, "completion_tokens": 200, "total_tokens": 1200}}).encode(), "application/json"
        self.send_response(infer_mode["status"])
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass


infer_fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["INFER_PORT"])), FakeVertex)
threading.Thread(target=infer_fake.serve_forever, daemon=True).start()


def infer_sign(keyfile, url=None):
    return subprocess.run([os.environ["SIGN"], "--key", keyfile, "POST", url or API + "/api/v1/inference"], capture_output=True, text=True, check=True).stdout.strip()


def infer(body, keyfile=os.environ["INFER_KEY"], auth=None):
    headers = {"Content-Type": "application/json"}
    if keyfile or auth:
        headers["Authorization"] = auth or "Nostr " + infer_sign(keyfile)
    return call("POST", "/api/v1/inference", json.dumps(body).encode(), headers)


def infer_pubkey(keyfile):
    return subprocess.run([os.environ["SIGN"], "--key", keyfile, "--pubkey"], capture_output=True, text=True, check=True).stdout.strip()


infer_ask = {"model": "google/gemini-3.8-flash", "messages": [{"role": "user", "content": "What is the capital of France?"}]}
infer_user = infer_pubkey(os.environ["INFER_KEY"])

check("inference: unsigned is a 401", infer(infer_ask, keyfile=None)[0] == 401)
check("inference: a GET is a 405", call("GET", "/api/v1/inference")[0] == 405)
check("inference: a signature for another URL is a 401", infer(infer_ask, auth="Nostr " + infer_sign(os.environ["INFER_KEY"], API + "/v1/other"))[0] == 401)
check("inference: a key not on the list is a 403", infer(infer_ask, keyfile=os.environ["OTHER_KEY"])[0] == 403)
s, b, _ = infer({"model": "gpt-6", "messages": infer_ask["messages"]})
check("inference: a model the route doesn't serve is a 400", s == 400 and b"does not serve" in b, b)
check("inference: no messages is a 400", infer({"model": infer_ask["model"]})[0] == 400)
check("inference: bad requests never reach Vertex", infer_seen == [])

s, b, h = infer(dict(infer_ask, max_tokens=100000, store=True, user="someone"))
check("inference: a signed request gets Vertex's answer", s == 200 and json.loads(b)["choices"][0]["message"]["content"] == "Paris.", b)
infer_sent = infer_seen[-1]
check("inference: the service's token goes to Vertex, not the caller's signature", infer_sent["auth"] == "Bearer test-token", infer_sent["auth"])
check("inference: Vertex gets the messages, its model name, the output capped at 8192, nothing stored",
      infer_sent["body"]["messages"] == infer_ask["messages"] and infer_sent["body"]["model"] == "google/gemini-3.8-flash" and infer_sent["body"]["max_tokens"] == 8192
      and "store" not in infer_sent["body"] and "user" not in infer_sent["body"], infer_sent["body"])
check("inference: the call is kept with its tokens and its cost at the card's rates",
      sql(f"select status||'|'||in_tokens||'|'||out_tokens||'|'||usd_micros from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "200|1000|200|1500")

s, b, h = infer(dict(infer_ask, stream=True))
infer_text = b.decode()
check("inference: a stream comes back as server-sent events", s == 200 and h.get("Content-Type") == "text/event-stream" and infer_text.endswith("data: [DONE]\n\n")
      and '"content": "Par"' in infer_text, (h, infer_text))
check("inference: a stream asks Vertex for its usage", infer_seen[-1]["body"].get("stream_options") == {"include_usage": True}, infer_seen[-1]["body"])
check("inference: a stream's usage, from its last chunk, is metered",
      sql(f"select in_tokens||'|'||out_tokens||'|'||usd_micros from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "2000|400|3000")

infer_mode["status"] = 429
s, b, _ = infer(infer_ask)
infer_mode["status"] = 200
check("inference: Vertex's refusal comes back as its status, so the Lab falls through to its next gateway", s == 429 and b"Resource exhausted" in b, (s, b))
check("inference: a refused call costs nothing", sql(f"select status||'|'||usd_micros from inference.calls where pubkey = '{infer_user}' order by id desc limit 1") == "429|0")

s, _, _ = infer(infer_ask, keyfile=os.environ["ADMIN_KEY"])
check("inference: an admin's key may use the route", s == 200)

infer_codes = [infer(infer_ask)[0] for _ in range(3)]
check("inference: an account's day is five requests here; the sixth is a 429 and never reaches Vertex", infer_codes == [200, 200, 429], infer_codes)
check("inference: the day's count and spend are the account's",
      sql(f"select n||'|'||usd_micros from inference.account_day where pubkey = '{infer_user}'") == "6|7500")
infer_admin = infer_pubkey(os.environ["ADMIN_KEY"])
sql(f"update inference.account_day set usd_micros = 2000000 where pubkey = '{infer_admin}'")
s, b, _ = infer(infer_ask, keyfile=os.environ["ADMIN_KEY"])
check("inference: an account whose day's spend is used up gets a 429", s == 429 and b"spend" in b, b)
