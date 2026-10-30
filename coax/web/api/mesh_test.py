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
        status = 503 if current == "503" else 403 if current == "403" else 302 if current == "redirect" else 200
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
check("oversized request is refused before authentication", call(b"x" * 262145)[0] == 413)
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
check("unknown model refuses", signed(body(model="unknown"))[0] == 400)
check("malformed JSON refuses", signed(b'{"model":')[0] == 400)
check("duplicate model refuses", signed(b'{"model":"elephant-1","model":"other"}')[0] == 400)
check("request cannot select another upstream or provider", signed(body(url="http://invalid.invalid", provider="not-ours"))[0] == 200 and seen[-1][0] == "/v1/systemone")
reset()
for value in ["model", "identity", "provider", "oversized", "redirect", "403"]:
    mode["value"] = value
    check(value + " substitution/refusal returns502", signed()[0] == 502)
mode["value"] = "503"
check("native unavailability refuses without substitution", signed()[0] == 503)
reset()
token = secrets.token_hex(32)
sql(f"INSERT INTO signin.sessions VALUES (projects.sha('{token}'),'{pubkey}',projects.now_ms(),projects.now_ms()+60000)")
cookie = {"Cookie": "oa_session=" + token}
check("cookie requires Origin", call(**cookie)[0] == 403)
check("cookie rejects cross-origin", call(**cookie, Origin="https://evil.invalid")[0] == 403)
check("valid site session and same Origin work", call(**cookie, Origin=API)[0] == 200)
check("explicit malformed authorization never falls back to cookie", call(auth="Bearer wrong", **cookie, Origin=API)[0] == 401)
check("signed request with foreign Origin refuses", (lambda data: call(data, sign(data), Origin="https://evil.invalid")[0])(body()) == 403)
sql(f"UPDATE signin.sessions SET expires_ms=0 WHERE token_sha256=projects.sha('{token}')")
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
fleet.shutdown()
print(f"mesh: {checks} scoped HTTP/database/upstream checks passed")
