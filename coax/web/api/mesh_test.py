"""Scoped production-route checks. Fresh keys, database and fake fleet only.

Run by mesh-test.sh. Python is an independent test client, never shipped.
"""
import concurrent.futures
import http.server
import json
import os
import secrets
import subprocess
import tempfile
import threading
import urllib.error
import urllib.request

API = os.environ["API"]
URL = API + "/api/v1/systemone"
PSQL = [os.environ["PSQL"], os.environ["PGURL"], "-Atc"]
MODEL = "elephant-1"
HASH = "0123456789abcdef" * 4
IDENTITY = MODEL + "@sha256:" + HASH
seen = []
mode = {"value": "ok"}
entered, release = threading.Event(), threading.Event()
checks = 0


def sql(query):
    return subprocess.run(PSQL + [query], check=True, capture_output=True, text=True).stdout.strip()


def check(name, value):
    global checks
    assert value, name
    checks += 1
    print("  ok  mesh: " + name)


def request(method="POST", body=None, headers=None):
    req = urllib.request.Request(URL, data=body, headers=headers or {}, method=method)
    try:
        with urllib.request.urlopen(req, timeout=12) as response:
            return response.status, response.read(), response.headers
    except urllib.error.HTTPError as response:
        return response.code, response.read(), response.headers


class Fleet(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        data = self.rfile.read(int(self.headers["Content-Length"]))
        seen.append((self.path, data, dict(self.headers)))
        current = mode["value"]
        if current == "wait":
            entered.set()
            release.wait(8)
        response = {"model": MODEL, "answers": {"wet": {"type": "choice", "choice": "yes", "confidence": 1.0,
                     "probabilities": {"yes": 1.0, "no": 0.0}}}, "service": {"identity": IDENTITY, "provider": HASH}}
        if current in ("model", "identity", "provider"):
            (response if current == "model" else response["service"])[current] = "substituted"
        status = 503 if current == "503" else 403 if current == "403" else 422 if current == "422" else 302 if current == "redirect" else 200
        raw = b"x" * 262145 if current == "oversized" else json.dumps(response).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Location", API + "/must-never-be-followed")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        try:
            self.wfile.write(raw)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, *args):
        pass


fleet = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["MESH_PORT"])), Fleet)
threading.Thread(target=fleet.serve_forever, daemon=True).start()

# A stand-in for Google's Decisions API (the Gemini door, mesh-gemini.coax).
google_seen = []
gmode = {"value": "ok"}
GOOGLE_ANSWER = {"answers": {"wet": {"choice": "yes", "confidence": 0.97, "probabilities": {"no": 0.03, "yes": 0.97}, "type": "choice"}},
                 "usage": {"total_tokens": 61, "total_input_tokens": 61, "input_tokens_by_modality": [{"modality": "text", "tokens": 61}], "total_output_tokens": 0}}
GOOGLE_REFUSALS = {"safety": (400, {"error": {"code": "safety", "message": "Request blocked due to safety violations."}}),
                   "badkey": (400, {"error": {"code": 400, "message": "API key not valid.", "status": "INVALID_ARGUMENT", "details": [{"reason": "API_KEY_INVALID"}]}}),
                   "500": (500, {"error": {"code": 500, "message": "internal", "status": "INTERNAL"}}),
                   "garbage": (200, {"answers": {}})}


class Google(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        data = self.rfile.read(int(self.headers["Content-Length"]))
        google_seen.append((self.path, data, dict(self.headers)))
        status, value = GOOGLE_REFUSALS.get(gmode["value"], (200, GOOGLE_ANSWER))
        raw = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def log_message(self, *args):
        pass


google = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["GEMINI_PORT"])), Google)
threading.Thread(target=google.serve_forever, daemon=True).start()
key = os.environ["MESH_KEY"]
pubkey = subprocess.run([os.environ["SIGN"], "--key", key, "--pubkey"], check=True, capture_output=True, text=True).stdout.strip()


def body(**overrides):
    value = {"model": MODEL, "state": "Water is wet.", "questions": {"wet": {"type": "choice", "criteria": {"yes": None, "no": None}}}, "nonce": secrets.token_hex(8)}
    value.update(overrides)
    return json.dumps(value).encode()


def sign(data=None, url=URL, age=0):
    env = os.environ.copy()
    env["SIGN_AGE"] = str(age)
    with tempfile.NamedTemporaryFile() as file:
        if data is not None:
            file.write(data)
            file.flush()
        args = [os.environ["SIGN"], "--key", key, "POST", url] + ([file.name] if data is not None else [])
        return "Nostr " + subprocess.run(args, env=env, check=True, capture_output=True, text=True).stdout.strip()


def call(data=None, auth=None, **headers):
    if data is None:
        data = body()
    headers = {"Content-Type": "application/json", **headers}
    if auth is not None:
        headers["Authorization"] = auth
    return request(body=data, headers=headers)


def signed(data=None):
    if data is None:
        data = body()
    return call(data, sign(data))


def reset():
    sql("TRUNCATE mesh.calls")
    mode["value"] = "ok"


check("unsigned is refused", call()[0] == 401)
check("GET is refused", request("GET")[0] == 405)
check("content type is required", request(body=body(), headers={"Content-Type": "text/plain"})[0] == 415)
check("oversized request is refused before authentication", call(b"x" * 16385)[0] == 413)
data = body()
check("NIP-98 payload is mandatory", call(data, sign())[0] == 401)
check("signature binds the exact body", call(body(), sign(data))[0] == 401)
check("signature binds exact URL", call(data, sign(data, API + "/api/v1/other"))[0] == 401)
check("stale signature refuses", call(data, sign(data, age=61))[0] == 401)
auth = sign(data)
status, response, headers = call(data, auth)
check("signed caller receives native result and exact identity", status == 200 and json.loads(response)["service"]["identity"] == IDENTITY)
check("native upstream receives body unchanged without user auth/cookies", seen[-1][0] == "/v1/systemone" and seen[-1][1] == data and "Authorization" not in seen[-1][2] and "Cookie" not in seen[-1][2])
check("response is private and varies by identity", "no-store" in headers["Cache-Control"] and "Authorization" in headers["Vary"])
check("signed request cannot replay", call(data, auth)[0] == 401)
reset()
large = json.loads(body(padding=""))
large["padding"] = "x" * (16384 - len(json.dumps(large).encode()))
large = json.dumps(large).encode()
check("exact16KiB request is admitted", len(large) == 16384 and signed(large)[0] == 200)
check("one byte above16KiB is refused", signed(large + b" ")[0] == 413)
check("unknown model refuses", signed(body(model="unknown"))[0] == 400)
check("malformed JSON refuses", signed(b'{"model":')[0] == 400)
check("duplicate model refuses", signed(b'{"model":"elephant-1","model":"other"}')[0] == 400)
check("request cannot select another upstream or provider", signed(body(url="http://invalid.invalid", provider="not-ours"))[0] == 200 and seen[-1][0] == "/v1/systemone")
reset()
for value in ["model", "identity", "provider", "oversized", "redirect", "403"]:
    mode["value"] = value
    check(value + " substitution/refusal returns502", signed()[0] == 502)
mode["value"] = "422"
status, response, _ = signed()
check("actual HTTP422 input rejection stays distinct from transport413", status == 422 and json.loads(response)["error"]["code"] == "unsupported_input")
mode["value"] = "503"
check("native unavailability refuses without substitution", signed()[0] == 503)
reset()
# A web session of ours (auth.c) for the key's account, made as /login makes one (coax/lib/crypto/token.coax's format).
import base64, hashlib, zlib
token = "oas_" + base64.b32encode(secrets.token_bytes(32)).decode().rstrip("=")
token += "".join("ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"[((zlib.crc32(token.encode()) >> 2) >> (25 - 5 * i)) & 31] for i in range(6))
mesh_acct = sql(f"SELECT acct FROM auth.account_for_key('{pubkey}', 'test')")
mesh_kid = sql(f"SELECT id FROM auth.nostr_keys WHERE pubkey = '{pubkey}'")
sql(f"SELECT auth.session_new('{hashlib.sha256(token.encode()).hexdigest()}', '{mesh_acct}', 'nostr', {mesh_kid}, 'L2', 'web', 'test', NULL)")
cookie = {"Cookie": "__Host-oa_session=" + token}
check("cookie requires Origin", call(**cookie)[0] == 403)
check("cookie rejects cross-origin", call(**cookie, Origin="https://evil.invalid")[0] == 403)
check("valid site session and same Origin work", call(**cookie, Origin=API)[0] == 200)
check("explicit malformed authorization never falls back to cookie", call(auth="Bearer wrong", **cookie, Origin=API)[0] == 401)
check("signed request with foreign Origin refuses", (lambda data: call(data, sign(data), Origin="https://evil.invalid")[0])(body()) == 403)
sql(f"SELECT auth.session_end(id) FROM auth.sessions WHERE token_sha256 = '{hashlib.sha256(token.encode()).hexdigest()}'")
check("expired session refuses", call(**cookie, Origin=API)[0] == 401)
reset()
for n, timestamp, expected in [(12, "clock_timestamp()", "minute"), (120, "clock_timestamp()-interval '2 minutes'", "account day")]:
    sql(f"INSERT INTO mesh.calls(pubkey,identity,started,finished) SELECT '{pubkey}','{IDENTITY}',{timestamp},true FROM generate_series(1,{n})")
    check(expected + " quota refuses", signed()[0] == 429)
    reset()
sql(f"INSERT INTO mesh.calls(pubkey,identity,finished) SELECT 'other','{IDENTITY}',true FROM generate_series(1,1200)")
check("global daily cap refuses", signed()[0] == 429)
reset()
sql(f"INSERT INTO mesh.calls(pubkey,identity) SELECT 'other','{IDENTITY}' FROM generate_series(1,4)")
check("global concurrency cap refuses", signed()[0] == 429)
sql("UPDATE mesh.calls SET lease_until=clock_timestamp()-interval '1 second'")
check("expired leases free concurrency", signed()[0] == 200)
reset()
mode["value"] = "wait"
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    first = pool.submit(signed)
    check("first task reached native fleet", entered.wait(3))
    check("same account cannot start second task while first runs", signed()[0] == 429)
    release.set()
    check("first task completes normally", first.result()[0] == 200)
check("completed calls release leases and retain status", sql("SELECT count(*) FROM mesh.calls WHERE finished AND status=200") == "1")
reset()
GEMINI_BODY = {"state": "Water is wet.", "questions": {"wet": {"type": "choice", "instructions": "Is water wet?", "criteria": {"yes": None, "no": None}}}}
GEMINI_SENT = (b'{"model":"gemini-decision-1","input":[{"type":"text","text":"Water is wet."}],"questions":{"wet":{"type":"choice",'
               b'"instructions":"Is water wet?","criteria":{"yes":"","no":""}}}}')


def gemini_body(model="gemini-decision-1", **overrides):
    value = dict(GEMINI_BODY, model=model, nonce=secrets.token_hex(8))
    value.update(overrides)
    return json.dumps(value).encode()


status, response, _ = signed(gemini_body())
answer = json.loads(response) if status == 200 else {}
check("a Gemini route answers in our contract", status == 200 and answer["model"] == "gemini-decision-1" and answer["answers"]["wet"]["choice"] == "yes"
      and answer["service"] == {"door": "gemini", "identity": "gemini-decision-1", "provider": "google"} and answer["usage"] == {"input_tokens": 61, "output_tokens": 0})
path, sent, sent_headers = google_seen[-1]
check("Google receives the mapped request byte for byte", path == "/v1beta/decisions" and sent == GEMINI_SENT)
check("Google receives the deployment's key and nothing of the caller's", sent_headers.get("x-goog-api-key") == os.environ["OA_GEMINI_DECISIONS_KEY"]
      and "Authorization" not in sent_headers and "Cookie" not in sent_headers)
reset()
one = json.loads(gemini_body()); one["questions"]["wet"]["criteria"] = {"yes": None}
before = len(google_seen)
check("a request Gemini Decision 1 can't take is refused before Google", signed(json.dumps(one).encode())[0] == 422 and len(google_seen) == before)
reset()
gmode["value"] = "safety"
status, response, _ = signed(gemini_body())
check("a safety block is a 422 that names it", status == 422 and json.loads(response)["error"]["code"] == "safety")
reset()
gmode["value"] = "badkey"
check("a refused key is the door being unavailable", signed(gemini_body())[0] == 503)
reset()
for value in ["500", "garbage"]:
    gmode["value"] = value
    check("Google's " + value + " is a 502", signed(gemini_body())[0] == 502)
    reset()
gmode["value"] = "ok"
status, response, _ = signed(gemini_body(model="auto"))
check("auto takes the first door in the order", status == 200 and json.loads(response)["model"] == "gemini-decision-1")
reset()
gmode["value"] = "500"
fleet_before = len(seen)
status, response, _ = signed(gemini_body(model="auto"))
check("auto hands a failed door's request to the next, renamed", status == 200 and json.loads(response)["service"]["identity"] == IDENTITY
      and len(seen) == fleet_before + 1 and json.loads(seen[-1][1])["model"] == MODEL)
check("the renamed body keeps every other byte", json.loads(seen[-1][1])["questions"] == GEMINI_BODY["questions"])
reset()
mode["value"] = "503"
check("auto refuses when every door fails", signed(gemini_body(model="auto"))[0] == 503)
reset()
gmode["value"] = "ok"
fleet.shutdown()
google.shutdown()
print(f"mesh: {checks} scoped HTTP/database/upstream checks passed")
