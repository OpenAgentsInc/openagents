# The runner (coax/web/agent/runner.c) against this oa-api, a local bare
# repository and the stand-in engine (coax/web/agent/test-engine.py): a wake
# runs one turn and streams its events, the turn's file edit is one pushed
# commit, UNDO TURN pushes its revert, STOP stops a turn, and no key reaches a
# log, an event or an answer. test.py runs this file after test_private_agent.py,
# whose fixture project, sessions and runner key it uses.
import shutil
import tempfile

RUNNER_BIN = os.environ["RUNNER_BIN"]
work = tempfile.mkdtemp()
bare = os.path.join(work, "remote.git")
seed = os.path.join(work, "seed")
G = {"GIT_AUTHOR_NAME": "Ada", "GIT_AUTHOR_EMAIL": "ada@example.com", "GIT_COMMITTER_NAME": "Ada", "GIT_COMMITTER_EMAIL": "ada@example.com",
     "PATH": "/usr/bin:/bin", "HOME": work}


def git(*args, cwd=None):
    return subprocess.run(["git", *args], cwd=cwd, env=G, capture_output=True, text=True, check=True).stdout


git("init", "-q", "--bare", "-b", "main", bare)
git("clone", "-q", bare, seed)
os.makedirs(os.path.join(seed, "project"))
open(os.path.join(seed, "project", "plan.md"), "w").write("# Plan\n")
git("add", "-A", cwd=seed)
git("commit", "-q", "-m", "start", cwd=seed)
git("push", "-q", "origin", "HEAD:main", cwd=seed)

RUNNER_SECRET = open(RUN_KEY).read().strip()
runner_port = int(os.environ["WAKE_PORT"]) + 1
runner_log = os.path.join(work, "runner.log")
runner = subprocess.Popen([RUNNER_BIN, "--listen", f"127.0.0.1:{runner_port}"], stderr=open(runner_log, "w"), env={
    "OA_API_ORIGIN": API, "OA_RUNNER_KEY": RUNNER_SECRET, "OA_AGENT_REPOS": f"{ASLUG}=file://{bare}=",
    "OA_AGENT_ENGINE": os.path.join(os.path.dirname(os.path.abspath(os.environ["RUNNER_BIN_SRC"])), "test-engine.py"),
    "ANTHROPIC_API_KEY": "test-anthropic-secret", "OA_AGENT_WORK": os.path.join(work, "turns"), "OA_GIT": shutil.which("git"),
    "PATH": "/usr/bin:/bin"})
for _ in range(50):
    try:
        if urllib.request.urlopen(f"http://127.0.0.1:{runner_port}/").status == 200:
            break
    except Exception:
        time.sleep(0.1)


def wake():
    req = urllib.request.Request(f"http://127.0.0.1:{runner_port}/wake", method="POST", data=json.dumps({"slug": ASLUG}).encode())
    return urllib.request.urlopen(req).status


def until(turn_id, statuses, seconds=30):
    for _ in range(seconds * 5):
        s, b = talk(ada)
        line = next((l for l in b.split("\n") if l.startswith(f"U|{turn_id}|")), "")
        if line.split("|")[3:4] and line.split("|")[3] in statuses:
            return b, line
        time.sleep(0.2)
    return b, line


s, st, _ = run("state", method="GET")
v_before = st["version"]
s, b, _ = page("POST", f"/projects/{ASLUG}/api/ask", {"text": "Mark the deck done\nand note it in the plan"}, cookie=ada, xff="10.1.0.2")
t = json.loads(b)["turn"]
check("the runner answers a wake at once", wake() == 202)
b, line = until(t, ("done", "failed"))
cols = line.split("|")
check("a wake runs one turn to the end", cols[3:4] == ["done"], (line, open(runner_log).read()[-2000:]))
check("the engine ran in its checkout with the model's key and without the runner's", "key:yes runner:no home:yes" in line, line)
log = git("log", "--format=%H %s", "main", cwd=bare)
head, subject = log.split("\n")[0].split(" ", 1)
check("the turn's file edit is one commit, named for the turn and the member's first line",
      subject == f"agent turn {t}: Mark the deck done" and len(log.strip().split("\n")) == 2, log)
check("the commit is the turn's, with its files", cols[9] == head[:7] and cols[10] == "1" and f"|agent.commit|{head[:7]} · agent turn {t}: Mark the deck done · project/plan.md" in b, b)
check("the turn's document edit streamed in as a card", "day 10-29: Start the deck → done" in b, b)
s, tj, _ = agent(os.environ["ADMIN_KEY"], "GET", "talk", slug=ASLUG, xff="10.1.0.1")
types = [e["type"] for e in tj["turns"][-1]["events"]]
check("the turn's events reached oa-api in order", types[:2] == ["user.message", "session.status_running"] and "agent.tool_use" in types
      and types[-1] == "session.status_idle", types)
check("its cost counts against the day", tj["spent"] == 0.05, tj["spent"])

# UNDO TURN: the document back, and the commit reverted and pushed
s, b, _ = page("POST", f"/projects/{ASLUG}/api/undo", {"turn": t}, cookie=cy, xff="10.1.0.3")
u = json.loads(b)["turn"]
wake()
b, line = until(u, ("done", "failed"))
check("UNDO TURN's revert turn ends done", line.split("|")[3:4] == ["done"], (line, open(runner_log).read()[-2000:]))
log = git("log", "--format=%s", "main", cwd=bare).split("\n")
check("UNDO TURN pushes the revert of the turn's commit", log[0] == f"agent turn {u}: undo turn {t}" and "deck is done" not in git("show", "main:project/plan.md", cwd=bare), log)
s, st, _ = run("state", method="GET")
check("and the document is the version from before the turn", st["doc"]["days"][0]["milestones"][0]["status"] == "todo")

# STOP
s, b, _ = page("POST", f"/projects/{ASLUG}/api/ask", {"text": "a slow one"}, cookie=ada, xff="10.1.0.2")
t2 = json.loads(b)["turn"]
wake()
b, line = until(t2, ("running",), 10)
s, _, _ = page("POST", f"/projects/{ASLUG}/api/stop", {"turn": t2}, cookie=cy, xff="10.1.0.3")
b, line = until(t2, ("stopped", "failed", "done"), 20)
check("STOP stops a running turn, which pushes nothing", line.split("|")[3:4] == ["stopped"] and git("log", "-1", "--format=%s", "main", cwd=bare).strip() == f"agent turn {u}: undo turn {t}", line)

runner.terminate()
runner.wait()
rlog = open(runner_log).read()
alog = open(os.environ["API_LOG"]).read()
s, everything = talk(ada)
for secret, name in ((RUNNER_SECRET, "the runner's key"), ("test-anthropic-secret", "the Anthropic key")):
    check(f"{name} is in no log, event or answer", secret not in rlog and secret not in alog and secret not in everything and secret not in json.dumps(tj))
check("the runner's log keeps no message text", "Mark the deck" not in rlog and "slow one" not in rlog, rlog[-500:])
shutil.rmtree(work, ignore_errors=True)
