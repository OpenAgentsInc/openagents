# openagents.com/chat (docs/chat.md, chat.c, migrations/0021_chat.sql, the
# runner's chat mode in coax/web/agent/chat-runner.c): only admins get in;
# a message wakes the runner, whose turn runs in a fresh read-only checkout
# with exactly one credential; a follow-up resumes the conversation's session
# with the model chosen for it; STOP stops; the daily cap on OpenAgents' key
# refuses; a person's own Anthropic API key or Claude subscription token is
# checked, sealed, used instead (no cap) and removed; and no credential shows
# up in an answer, the database's plain columns or a log. test.py runs this
# file last; ADMIN_PUB, OTHER, new_key, call, check and sql come from the
# files before it. The runner gets a stand-in engine (test-chat-engine.py),
# a fake models list and a fake claude CLI.
import hashlib
import http.server
import shutil
import tempfile
import threading

CHAT_PORT = int(os.environ["CHAT_PORT"])
cwork = tempfile.mkdtemp()
OURS = "sk-ant-api03-OURS-test-" + "o" * 40
GOOD_API = "sk-ant-api03-good-" + "a" * 40 + "WXYZ"
BAD_API = "sk-ant-api03-bad-" + "b" * 40
GOOD_OAT = "sk-ant-oat01-good-" + "c" * 40 + "OATK"
BAD_OAT = "sk-ant-oat01-bad-" + "d" * 40
SECRETS = [OURS, GOOD_API, BAD_API, GOOD_OAT, BAD_OAT]
h8 = lambda v: hashlib.sha256(v.encode()).hexdigest()[:8]


class FakeAnthropic(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        ok = self.path.startswith("/v1/models") and self.headers.get("x-api-key") == GOOD_API
        body = b'{"data":[{"id":"claude-haiku-5-5"}]}' if ok else b'{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}'
        self.send_response(200 if ok else 401)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *a):
        pass


anthropic_http = http.server.ThreadingHTTPServer(("127.0.0.1", CHAT_PORT + 1), FakeAnthropic)
threading.Thread(target=anthropic_http.serve_forever, daemon=True).start()

fake_claude = os.path.join(cwork, "claude")
with open(fake_claude, "w") as f:
    f.write("#!/bin/sh\n"
            "if [ -n \"$ANTHROPIC_API_KEY$ANTHROPIC_AUTH_TOKEN\" ]; then echo '{\"is_error\":true,\"result\":\"an API key leaked into the check\"}'; exit 1; fi\n"
            f"if [ \"$CLAUDE_CODE_OAUTH_TOKEN\" = \"{GOOD_OAT}\" ]; then echo '{{\"is_error\":false,\"result\":\"ok\"}}'; exit 0; fi\n"
            "echo '{\"is_error\":true,\"result\":\"Failed to authenticate. API Error: 401 OAuth access token is invalid.\"}'; exit 1\n")
os.chmod(fake_claude, 0o755)

# the public repository, as a local bare one
cbare, cseed = os.path.join(cwork, "openagents.git"), os.path.join(cwork, "seed")
CG = {"GIT_AUTHOR_NAME": "Ada", "GIT_AUTHOR_EMAIL": "a@example.com", "GIT_COMMITTER_NAME": "Ada", "GIT_COMMITTER_EMAIL": "a@example.com", "PATH": "/usr/bin:/bin", "HOME": cwork}
cgit = lambda *a, cwd=None: subprocess.run(["git", *a], cwd=cwd, env=CG, capture_output=True, text=True, check=True).stdout
cgit("init", "-q", "--bare", "-b", "main", cbare)
cgit("clone", "-q", cbare, cseed)
open(os.path.join(cseed, "README.md"), "w").write("first\n")
open(os.path.join(cseed, "big.png"), "wb").write(b"\x89PNG" + b"0" * 1000)
cgit("add", "-A", cwd=cseed); cgit("commit", "-q", "-m", "one", cwd=cseed); cgit("push", "-q", "origin", "HEAD:main", cwd=cseed)

agent_src = os.path.dirname(os.path.abspath(os.environ["RUNNER_BIN_SRC"]))
chat_log = os.path.join(cwork, "runner.log")
chat_runner = subprocess.Popen([os.environ["RUNNER_BIN"], "--listen", f"127.0.0.1:{CHAT_PORT}"], stderr=open(chat_log, "w"), env={
    "OA_API_ORIGIN": API, "OA_RUNNER_KEY": open(os.environ["RUN_KEY_FILE"]).read().strip(), "OA_CHAT_SEAL_KEY": "5e" * 32,
    "OA_CHAT_ENGINE": os.path.join(agent_src, "test-chat-engine.py"), "OA_ANTHROPIC_URL": f"http://127.0.0.1:{CHAT_PORT + 1}",
    "OA_CLAUDE": fake_claude, "OA_CHAT_REPO": "file://" + cbare, "OA_CHAT_SESSIONS": os.path.join(cwork, "sessions"),
    "OA_AGENT_WORK": os.path.join(cwork, "work"), "ANTHROPIC_API_KEY": OURS, "OA_GIT": shutil.which("git"), "PATH": "/usr/bin:/bin"})
for _ in range(50):
    try:
        if urllib.request.urlopen(f"http://127.0.0.1:{CHAT_PORT}/").status == 200:
            break
    except Exception:
        time.sleep(0.1)


def chat_signed(keyfile, method, path, raw=None):
    args = [SIGN_BIN, "--key", keyfile, method, API + path]
    if raw is not None:
        f = tempfile.NamedTemporaryFile(delete=False); f.write(raw); f.close(); args.append(f.name)
    return "Nostr " + subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()


# one sign-in (a signed request is good once), its three cookies read from the raw headers
req = urllib.request.Request(API + "/api/admin/login?auth=" + urllib.parse.quote(chat_signed(os.environ["ADMIN_KEY"], "GET", "/api/admin/login")[6:], safe="") + "&next=chat")
cookies, where, s = [], None, 0
try:
    r = urllib.request.urlopen(req); s, cookies, where = r.status, r.headers.get_all("Set-Cookie") or [], r.headers.get("Location")
except urllib.error.HTTPError as e:
    s, cookies, where = e.code, e.headers.get_all("Set-Cookie") or [], e.headers.get("Location")
check("chat: admin sign-in with next=chat goes to /chat", s == 303 and where == "/chat", (s, where))
CTOK = next((c.split(";")[0].split("=", 1)[1] for c in cookies if "Path=/api/chat" in c), "")
check("chat: the admin's cookie opens /api/chat too", len(CTOK) == 64 and any("Path=/api/chat" in c and "HttpOnly" in c and "SameSite=Strict" in c for c in cookies), cookies)


def cpage(method, op, body=None, cookie=None, origin=API, query=""):
    h = {"X-Forwarded-For": "10.9.0.1"}
    if origin: h["Origin"] = origin
    if cookie is not False: h["Cookie"] = "oa_admin=" + (cookie or CTOK)
    raw = None if body is None else json.dumps(body).encode()
    if raw is not None: h["Content-Type"] = "application/json"
    s, b, _ = call(method, "/api/chat/" + op + query, raw, h)
    return s, b.decode()


def state(c=None):
    s, b = cpage("GET", "state", query=f"?c={c}" if c else "")
    return s, b


def lines(b, kind):
    return [l.split("|") for l in b.split("\n") if l.startswith(kind + "|")]


def turn_line(b, tid):
    return next((l for l in lines(b, "U") if l[1] == str(tid)), None)


def until_turn(c, tid, statuses, seconds=30):
    for _ in range(seconds * 5):
        s, b = state(c)
        u = turn_line(b, tid)
        if u and u[2] in statuses:
            return b, u
        time.sleep(0.2)
    return b, u


# ---- who gets in
s, b, _ = call("GET", "/api/chat/state")
check("chat: signed out, a 401 like the dashboard's", s == 401 and json.loads(b) == {"error": "sign in as an admin"}, (s, b))
s, b, _ = call("GET", "/api/chat/state", headers={"Authorization": chat_signed(os.environ["OTHER_KEY"], "GET", "/api/chat/state")})
check("chat: someone not an admin, a 403 and nothing else", s == 403 and json.loads(b) == {"error": "admins only"}, (s, b))
s, b = cpage("POST", "send", {"text": "hi"}, cookie="0" * 64)
check("chat: a made-up session can't send", s == 401, (s, b))
s, b = cpage("POST", "send", {"text": "hi"}, origin="https://evil.example")
check("chat: a write from another site is refused", s == 403 and "cross-site" in b, (s, b))
s, b, _ = call("POST", "/api/chat/runner/next", b"{}", {"Authorization": chat_signed(os.environ["ADMIN_KEY"], "POST", "/api/chat/runner/next", b"{}")})
check("chat: only the runner takes turns", s == 403, (s, b))
s, b = state()
check("chat: an admin's empty state", s == 200 and b.startswith("Z|0.00|1.00|||0|0|0"), (s, b))

# ---- a conversation: an answer from the checkout, then a follow-up that resumes on another model
s, b = cpage("POST", "send", {"text": "What does README.md say?", "model": "claude-haiku-5-5"})
r = json.loads(b) if s == 200 else {}
C, T1 = r.get("c"), r.get("turn")
check("chat: a message makes a conversation and a turn", s == 200 and C and T1 and r.get("model") == "claude-haiku-5-5", (s, b))
b, u = until_turn(C, T1, ("done", "failed"))
check("chat: the turn ran in a fresh checkout of main, on our key, as a chat workspace",
      u and u[2] == "done" and f"cred=ANTHROPIC_API_KEY:{h8(OURS)} " in u[7] and "readme=first" in u[7] and "mode=chat" in u[7]
      and "model=claude-haiku-5-5" in u[7] and "resume=no" in u[7] and u[5] == "openagents" and "others=none" in u[7], u)
check("chat: the answer keeps its Markdown line breaks escaped", u and u[7].startswith("## Answer\\n\\n"), u)
check("chat: its tool use is shown", any(t[1] == str(T1) and t[2] == "Read" and t[3] == "README.md" for t in lines(b, "T")), b)
check("chat: the conversation is listed with its title and model", any(l[1] == str(C) and l[2] == "What does README.md say?" and l[3] == "claude-haiku-5-5" for l in lines(b, "C")), b)
check("chat: big binary assets are left out of the checkout", os.path.exists(os.path.join(cwork, "work", "chat-repo", "README.md"))
      and not os.path.exists(os.path.join(cwork, "work", "chat-repo", "big.png")))
check("chat: the session file is kept for the next turn", sql(f"select session_id from chat.conversations where id = {C}") + ".jsonl" in os.listdir(os.path.join(cwork, "sessions")))

open(os.path.join(cseed, "README.md"), "w").write("second\n")
cgit("commit", "-qam", "two", cwd=cseed); cgit("push", "-q", "origin", "HEAD:main", cwd=cseed)
s, b = cpage("POST", "model", {"c": str(C), "model": "claude-sonnet-5-5"})
check("chat: the model changes mid-conversation", s == 200, (s, b))
s, b = cpage("POST", "model", {"c": str(C), "model": "gpt-5"})
check("chat: only the three Claude models are accepted", s == 400, (s, b))
s, b = cpage("POST", "send", {"c": str(C), "text": "And now?"})
T2 = json.loads(b).get("turn") if s == 200 else None
b, u = until_turn(C, T2, ("done", "failed"))
check("chat: the follow-up resumes the same session, on the new model, in a refreshed checkout",
      u and u[2] == "done" and "resume=yes" in u[7] and "turns=1" in u[7] and "model=claude-sonnet-5-5" in u[7] and "readme=second" in u[7] and u[4] == "claude-sonnet-5-5", u)
check("chat: a turn records that it resumed", sql(f"select resumed from chat.turns where id = {T2}") == "t")
s, b = cpage("POST", "send", {"c": str(C), "text": "third", "model": "claude-opus-5-5"})
T3 = json.loads(b).get("turn") if s == 200 else None
b, u = until_turn(C, T3, ("done", "failed"))
check("chat: a model given with the message is used and kept", u and "model=claude-opus-5-5" in u[7] and "turns=2" in u[7]
      and lines(b, "S")[0][3] == "claude-opus-5-5", u)

# ---- STOP
s, b = cpage("POST", "send", {"c": str(C), "text": "slow one"})
T4 = json.loads(b).get("turn") if s == 200 else None
b, u = until_turn(C, T4, ("running",))
s2, b2 = cpage("POST", "send", {"c": str(C), "text": "while busy"})
check("chat: one turn at a time in a conversation", s2 == 409, (s2, b2))
s, b = cpage("POST", "stop", {"c": str(C)})
b, u = until_turn(C, T4, ("stopped", "failed", "done"))
check("chat: STOP stops the running turn", s == 200 and u and u[2] == "stopped", (s, u))

# ---- the cap on our key ($1 here)
s, b = cpage("POST", "send", {"c": str(C), "text": "costly"})
T5 = json.loads(b).get("turn") if s == 200 else None
b, u = until_turn(C, T5, ("done", "failed"))
check("chat: the turn's budget is what's left of the cap", u and "budget=0.93" in u[7], u)
s, b = cpage("POST", "send", {"c": str(C), "text": "one more"})
check("chat: over the cap, OpenAgents' key refuses, and says so", s == 429 and "limit for /chat" in b and json.loads(b).get("cap") is True, (s, b))
s, b = state(C)
check("chat: the page sees the spend at the cap", s == 200 and lines(b, "Z")[0][1] == "1.07" and lines(b, "Z")[0][2] == "1.00", b)

# ---- your own key
s, b = cpage("POST", "key", {"credential": "not-a-key-at-all-not-a-key"})
check("chat: something that isn't a key is refused", s == 400 and "That key didn't work" in b, (s, b))
s, b = cpage("POST", "key", {"credential": BAD_API})
check("chat: a bad API key is refused with the reason, and nothing is saved",
      s == 400 and "That key didn't work: invalid x-api-key" in b and sql("select count(*) from chat.credentials") == "0", (s, b))
s, b = cpage("POST", "key", {"credential": BAD_OAT})
check("chat: a bad subscription token is refused with the reason", s == 400 and "That key didn't work" in b and "OAuth access token is invalid" in b, (s, b))
s, b = cpage("POST", "key", {"credential": GOOD_API})
check("chat: a good API key is checked and saved; only its kind and last four come back",
      s == 200 and json.loads(b) == {"kind": "api", "label": "Anthropic API key", "last4": "WXYZ"} and GOOD_API not in b, (s, b))
s, b = state(C)
check("chat: the page shows the key's kind and last four only", lines(b, "Z")[0][3:6] == ["api", "WXYZ", "0"] and GOOD_API not in b, b)
s, b = cpage("POST", "send", {"c": str(C), "text": "with my key"})
T6 = json.loads(b).get("turn") if s == 200 else None
b, u = until_turn(C, T6, ("done", "failed"))
check("chat: with your own key the cap doesn't apply, and the turn uses only your key",
      s == 200 and u and u[2] == "done" and f"cred=ANTHROPIC_API_KEY:{h8(GOOD_API)} " in u[7] and u[5] == "own" and "budget=0.00" in u[7], (s, u))
s, b = cpage("POST", "key", {"credential": GOOD_OAT})
check("chat: a good subscription token is checked with a CLI turn and replaces the API key",
      s == 200 and json.loads(b)["kind"] == "oauth" and json.loads(b)["label"] == "Claude subscription token (from claude setup-token)", (s, b))
s, b = cpage("POST", "send", {"c": str(C), "text": "with my token"})
T7 = json.loads(b).get("turn") if s == 200 else None
b, u = until_turn(C, T7, ("done", "failed"))
check("chat: a subscription token goes in CLAUDE_CODE_OAUTH_TOKEN alone", u and u[2] == "done" and f"cred=CLAUDE_CODE_OAUTH_TOKEN:{h8(GOOD_OAT)} " in u[7], u)
s, b = cpage("POST", "send", {"c": str(C), "text": "expire now"})
T8 = json.loads(b).get("turn") if s == 200 else None
b, u = until_turn(C, T8, ("done", "failed"))
check("chat: an expired token says so and is marked expired", u and "setup-token" in u[8] and lines(b, "Z")[0][5] == "1", (u, lines(b, "Z")))
s, b = cpage("POST", "key_remove", {})
s2, b2 = state(C)
check("chat: REMOVE deletes the saved key", s == 200 and sql("select count(*) from chat.credentials") == "0" and lines(b2, "Z")[0][3] == "", (s, b2))

# ---- another admin sees none of it (the same 'admins only' rules, per person)
s, b = cpage("GET", "state", query=f"?c={C}", cookie="f" * 64)
check("chat: someone else's session can't read the conversation", s == 401, s)

# ---- the project agent: the member who asked, on their own saved credential
# (one store with /chat), else OpenAgents' key under the project's cap; never another member's
prunner_log = os.path.join(cwork, "prunner.log")
prunner = subprocess.Popen([os.environ["RUNNER_BIN"], "--listen", f"127.0.0.1:{CHAT_PORT + 2}"], stderr=open(prunner_log, "w"), env={
    "OA_API_ORIGIN": API, "OA_RUNNER_KEY": open(os.environ["RUN_KEY_FILE"]).read().strip(), "OA_CHAT_SEAL_KEY": "5e" * 32,
    "OA_AGENT_ENGINE": os.path.join(agent_src, "test-chat-engine.py"), "OA_AGENT_REPOS": "",
    "OA_AGENT_WORK": os.path.join(cwork, "pwork"), "ANTHROPIC_API_KEY": OURS, "OA_GIT": shutil.which("git"), "PATH": "/usr/bin:/bin"})
for _ in range(50):
    try:
        if urllib.request.urlopen(f"http://127.0.0.1:{CHAT_PORT + 2}/").status == 200:
            break
    except Exception:
        time.sleep(0.1)


def pturn(cookie, text, xff):
    s, b, _ = page("POST", f"/projects/{ASLUG}/api/ask", {"text": text}, cookie=cookie, xff=xff)
    tid = json.loads(b).get("turn") if s == 200 else None
    urllib.request.urlopen(urllib.request.Request(f"http://127.0.0.1:{CHAT_PORT + 2}/wake", method="POST", data=json.dumps({"slug": ASLUG}).encode()))
    for _ in range(150):
        s, tb = talk(cookie)
        u = next((l.split("|") for l in tb.split("\n") if l.startswith(f"U|{tid}|")), None)
        if u and u[3] in ("done", "failed", "stopped", "refused"):
            return u, tb
        time.sleep(0.2)
    return u, tb


def kline(tb):
    return next((l.split("|") for l in tb.split("\n") if l.startswith("K|")), None)


s, b, _ = page("POST", f"/projects/{ASLUG}/api/key", {"credential": GOOD_OAT}, cookie=cy, xff="10.1.0.3")
check("project: a member saves a subscription token from the project page", s == 200 and json.loads(b).get("kind") == "oauth" and GOOD_OAT not in b.decode(), (s, b))
s, b, _ = page("POST", f"/projects/{ASLUG}/api/key", {"credential": GOOD_OAT}, cookie=None, xff="10.1.0.8")
check("project: KEY needs a member", s == 404, s)
u, tb = pturn(cy, "Cy's question", "10.1.0.3")
check("project: Cy's turn runs on Cy's token alone, no cap from us",
      u and u[3] == "done" and f"cred=CLAUDE_CODE_OAUTH_TOKEN:{h8(GOOD_OAT)} " in u[7] and u[15] == "own" and "budget=0.00" in u[7], u)
check("project: Cy's panel shows Cy's key", kline(tb) == ["K", "oauth", "OATK", "0"], kline(tb))
u, tb = pturn(ada, "Ada's question", "10.1.0.2")
check("project: Ada has none, so hers runs on OpenAgents' key under the cap, never Cy's",
      u and u[3] == "done" and f"cred=ANTHROPIC_API_KEY:{h8(OURS)} " in u[7] and h8(GOOD_OAT) not in u[7] and u[15] == "openagents" and "budget=0.00" not in u[7], u)
check("project: Ada's panel shows no key of hers", kline(tb) == ["K", "", "", "0"], kline(tb))
s, b = cpage("POST", "key", {"credential": GOOD_API})
u, tb = pturn(ada, "Ada again", "10.1.0.2")
check("project: a key saved on /chat runs Ada's project turns too (one store)", s == 200 and u and f"cred=ANTHROPIC_API_KEY:{h8(GOOD_API)} " in u[7] and u[15] == "own", (s, u))
spent_before = sql(f"select projects.spent('{ASLUG}')")
u, tb = pturn(cy, "costly on Cy's own", "10.1.0.3")
check("project: turns on a member's own credential don't count toward the project's spend", sql(f"select projects.spent('{ASLUG}')") == spent_before, spent_before)
u, tb = pturn(cy, "expire please", "10.1.0.3")
check("project: an expired token says so and is marked expired; the next turn falls back to our key",
      u and u[3] == "failed" and "setup-token" in u[8] and kline(tb) == ["K", "oauth", "OATK", "1"], (u, kline(tb)))
s, b, _ = page("POST", f"/projects/{ASLUG}/api/key_remove", {}, cookie=cy, xff="10.1.0.3")
s2, tb = talk(cy)
check("project: REMOVE from the project page deletes it", s == 200 and kline(tb) == ["K", "", "", "0"], (s, kline(tb)))
cpage("POST", "key_remove", {})
prunner.terminate()
time.sleep(0.3)
chat_log_extra = open(prunner_log).read()

# ---- no credential anywhere it shouldn't be
time.sleep(0.5)
logs = open(os.environ["API_LOG"]).read() + open(chat_log).read() + chat_log_extra
dump = sql("select coalesce(string_agg(t::text, ' '), '') from (select row_to_json(x)::text t from chat.turns x union all select row_to_json(e)::text from chat.events e "
           "union all select row_to_json(c)::text from chat.conversations c union all select row_to_json(p)::text from projects.turns p "
           "union all select row_to_json(q)::text from projects.events q) z")
leaked = [h8(v) for v in SECRETS if v in logs or v in dump]
check("chat: no key or token in the logs, turns, events or conversations", not leaked, leaked)
chat_runner.terminate()
