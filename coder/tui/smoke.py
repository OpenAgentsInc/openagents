#!/usr/bin/env python3
"""Drives coder/tui/coder.sh in a real pseudo-terminal (100x30) on the
offline echo engine: types a message and checks the streamed reply, runs
"$ echo ..." and checks the command row, pastes two lines with bracketed
paste, walks history with Up, switches agent with Shift-Tab (legacy and
kitty), then quits with Ctrl-C and checks the exit. Then the verbs without a
terminal, in a fresh HOME: coder run --json on the psionic engine against a
stand-in local model server (local_model.py), coder ls --json, coder log and
coder status on an unknown ID, a progress bar's output, a plugin's command, coder fork, coder run --detach then coder
stop, and coder run without --engine or a System
One key (the default engine, with the reason in the log).

    python3 coder/tui/smoke.py                # offline, echo engine
    python3 coder/tui/smoke.py --live         # two turns on the claude engine (your Claude Code login)
    python3 coder/tui/smoke.py --live codex   # the same on the codex engine (your Codex login)
    python3 coder/tui/smoke.py --live microcoder   # ... on Microcoder (keys under ~/work/.secrets)

Test-only harness; the program under test is Coax.
"""
import json, os, pty, re, select, signal, struct, subprocess, sys, tempfile, time, fcntl, termios

HERE = os.path.dirname(os.path.abspath(__file__))
STYLE = re.compile(rb"\x1b\[[0-9;?<>]*[@-~]")


def shown(out):
    """The output without style sequences: what the text says."""
    return STYLE.sub(b"", bytes(out))
REPO = os.path.dirname(os.path.dirname(HERE))


def spawn(engine):
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(REPO)
        env = dict(os.environ, TERM="xterm-256color", BROWSER="/usr/bin/true", NO_OPEN_BROWSER="1")
        os.execve(os.path.join(HERE, "coder.sh"), [os.path.join(HERE, "coder.sh"), "--engine", engine], env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
    return pid, fd


def live(engine):
    """Two turns in one session: the second needs the first."""
    pid, fd = spawn(engine)
    out = bytearray()

    def read_until(needle, seconds):
        end = time.time() + seconds
        while time.time() < end and needle.encode() not in shown(out):
            r, _, _ = select.select([fd], [], [], 0.1)
            if r:
                try:
                    out.extend(os.read(fd, 65536))
                except OSError:
                    break
        return needle.encode() in shown(out)

    steps = [("banner", read_until("/help for commands.", 600))]
    os.write(fd, b"Remember the word heliotrope. Reply with only: noted\r")
    steps.append(("first turn", read_until("● noted", 120) and read_until("└ finished", 30)))
    os.write(fd, b"What word did I ask you to remember? Reply with only that word.\r")
    steps.append(("second turn remembers", read_until("● heliotrope", 120)))
    end = time.time() + 30
    while out.count("└ finished".encode()) < 2 and time.time() < end:
        read_until("\x00never", 0.2)
    steps.append(("second turn ended", out.count("└ finished".encode()) >= 2))
    os.write(fd, b"\x1b[Z")                 # Shift-Tab: the other agent
    after = {"claude": "Agent: Codex", "codex": "Agent: Microcoder", "microcoder": "Agent: Claude Code"}[engine]
    steps.append(("Shift-Tab switches agent", read_until(after, 5)))
    for _ in range(3):                      # stop, clear, quit: whichever applies
        os.write(fd, b"\x03")
        time.sleep(0.5)
    status = None
    end = time.time() + 15
    while status is None and time.time() < end:
        p, st = os.waitpid(pid, os.WNOHANG)
        if p == pid:
            status = st
        read_until("\x00never", 0.2)
    if status is None:
        os.kill(pid, signal.SIGKILL)
    steps.append(("quits", status is not None))
    for name, good in steps:
        print(("ok   " if good else "FAIL ") + name)
    good = all(g for _, g in steps)
    if not good:
        sys.stdout.write(out[-3000:].decode("utf-8", "replace") + "\n")
    print("live smoke: " + ("ok" if good else "FAILED"))
    sys.exit(0 if good else 1)


GH_STUB = r"""
import json, os, sys
d = os.environ["GH_STUB_DIR"]
a = sys.argv[1:]
if a[:2] == ["api", "user"]:
    print("alice")
elif a[:1] == ["api"]:
    if "/actions/runs" in a[1]:
        print('{"k":"run:4242","b":"main","a":"ci-bot","t":"CI failed: tests on main: Fix parser\\nhttps://github.com/c/i/actions/runs/4242"}')
    elif "/issues/comments" in a[1]:
        print('{"k":"comment:555","c":"555","n":7,"a":"alice","t":"thanks"}')
        if os.path.exists(os.path.join(d, "gh-posted")):
            print(json.dumps({"k": "comment:999", "c": "999", "n": 7, "a": "alice", "t": open(os.path.join(d, "gh-posted")).read().split(" --body ", 1)[-1]}))
    elif "/issues/7/comments" in a[1]:
        # the Router looking for a reply it posted before a crash: the marker it names is in the posted body
        body = open(os.path.join(d, "gh-posted")).read() if os.path.exists(os.path.join(d, "gh-posted")) else ""
        want = a[3].split('contains("', 1)[-1].split('"', 1)[0] if len(a) > 3 else ""
        if want and want in body:
            print("999")
    else:
        print('{"k":"issue:7","n":7,"a":"alice","t":"$ echo gh-ok"}')
elif a[:2] == ["issue", "comment"]:
    open(os.path.join(d, "gh-posted"), "w").write(" ".join(a))
    open(os.path.join(d, "gh-posts"), "a").write("post\n")
    print("https://github.com/o/r/issues/7#issuecomment-999")
else:
    sys.exit(1)
"""


def verbs(check):
    """run, ls, log and status, with no terminal, on the psionic engine."""
    home = tempfile.mkdtemp(prefix="coder-smoke-")
    server = subprocess.Popen([sys.executable, os.path.join(HERE, "local_model.py")], stdout=subprocess.PIPE, text=True)
    try:
        port = server.stdout.readline().strip()
        env = dict(os.environ, HOME=home, PSIONIC_BASE_URL="http://127.0.0.1:" + port, BROWSER="/usr/bin/true")
        coder = os.path.join(HERE, "coder.sh")
        run = subprocess.run([coder, "run", "say ok", "--engine", "psionic", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=600)
        lines = [json.loads(l) for l in run.stdout.splitlines() if l.startswith("{")]
        kinds = [l["kind"] for l in lines]
        check("run --json exits 0", run.returncode == 0)
        check("run --json: session, user, request, tool, result, reply, end", kinds == ["session", "user", "request", "tool", "result", "reply", "end"])
        check("run --json: the local model answered", any(l.get("text") == "local-ok" for l in lines) and lines[-1].get("ending") == "finished")
        ls = subprocess.run([coder, "ls", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        rows = json.loads(ls.stdout) if ls.returncode == 0 else []
        check("ls --json lists the session", len(rows) == 1 and rows[0]["engine"] == "psionic" and rows[0]["state"] == "finished" and rows[0]["steps"] == 1)
        sid = rows[0]["id"] if rows else "none"
        log = subprocess.run([coder, "log", sid[:15]], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        check("log by ID prefix", log.returncode == 0 and "● local-ok" in log.stdout and "Bash(echo local-step)" in log.stdout)
        # the effort reaches the agent's own flag (chapter 13); a stand-in claude records its arguments
        fake = os.path.join(home, "fakebin"); os.makedirs(fake, exist_ok=True)
        with open(os.path.join(fake, "claude"), "w") as f:
            f.write("#!/bin/sh\necho \"$@\" > " + os.path.join(fake, "args") + "\nexit 1\n")
        os.chmod(os.path.join(fake, "claude"), 0o755)
        subprocess.run([coder, "run", "hi", "--engine", "claude", "--effort", "high"], cwd=REPO, env=dict(env, PATH=fake + ":" + env["PATH"]), capture_output=True, text=True, timeout=120)
        check("--effort reaches Claude Code", os.path.exists(os.path.join(fake, "args")) and "--effort=high" in open(os.path.join(fake, "args")).read())
        bad = subprocess.run([coder, "run", "hi", "--effort", "huge"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        check("--effort takes low, medium or high", bad.returncode == 2 and "--effort is low, medium or high" in bad.stderr)
        # approvals answered by policy (chapters 10 and 11): a stand-in claude asks three times
        stub = os.path.join(home, "stubbin"); os.makedirs(stub, exist_ok=True)
        with open(os.path.join(stub, "claude"), "w") as f:
            f.write("#!/bin/sh\nexec " + sys.executable + " " + os.path.join(HERE, "claude_stub.py") + " \"$@\"\n")
        os.chmod(os.path.join(stub, "claude"), 0o755)
        work = os.path.realpath(tempfile.mkdtemp(prefix="coder-policy-")); answers = os.path.join(home, "answers")
        pol = subprocess.run([coder, "run", "go", "--engine", "claude", "--allow", "write", "--ceiling", "write", "--cwd", work, "--json"], cwd=REPO,
                             env=dict(env, PATH=stub + ":" + env["PATH"], CLAUDE_STUB_OUT=answers), capture_output=True, text=True, timeout=120)
        verdicts = [(l["tool"], l["verdict"]) for l in (json.loads(x) for x in pol.stdout.splitlines() if '"approval"' in x)]
        check("policy: write inside runs, outside asks, a command above the ceiling is refused",
              verdicts == [("Write(a.txt)", "run"), ("Write(/etc/stub-outside.txt)", "ask"), ("Bash(touch b.txt)", "refuse")])
        sent = [json.loads(x)["behavior"] for x in open(answers)] if os.path.exists(answers) else []
        check("policy: Claude Code gets allow, then deny with no one to ask, then deny", sent == ["allow", "deny", "deny"] and '"ending":"finished"' in pol.stdout)
        # the same asks through coder acp (chapter 16): the editor is asked with session/request_permission, and its answer reaches Claude Code
        acp_answers = os.path.join(home, "acp-answers")
        pa = subprocess.Popen([coder, "acp", "--engine", "claude", "--allow", "write", "--ceiling", "write"], cwd=REPO, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1,
                              env=dict(env, PATH=stub + ":" + env["PATH"], CLAUDE_STUB_OUT=acp_answers))
        def pa_send(o):
            pa.stdin.write(json.dumps(o) + "\n"); pa.stdin.flush()
        pa_send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1, "clientCapabilities": {}}})
        pa_send({"jsonrpc": "2.0", "id": 2, "method": "session/new", "params": {"cwd": work, "mcpServers": []}})
        pa_send({"jsonrpc": "2.0", "id": 3, "method": "session/prompt", "params": {"sessionId": "s1", "prompt": [{"type": "text", "text": "go"}]}})
        pgot, pend_t = [], time.time() + 90
        while time.time() < pend_t:
            r, _, _ = select.select([pa.stdout], [], [], 0.2)
            if not r: continue
            line = pa.stdout.readline()
            if not line: break
            m = json.loads(line); pgot.append(m)
            if m.get("method") == "session/request_permission":
                pa_send({"jsonrpc": "2.0", "id": m["id"], "result": {"outcome": {"outcome": "selected", "optionId": "allow_once"}}})
            if m.get("id") == 3: break
        pa.stdin.close()
        try:
            pa.wait(15)
        except subprocess.TimeoutExpired:
            pa.kill()
        preqs = [m for m in pgot if m.get("method") == "session/request_permission"]
        psent = [json.loads(x)["behavior"] for x in open(acp_answers)] if os.path.exists(acp_answers) else []
        check("acp: an ask goes to the editor as session/request_permission, and its allow reaches Claude Code",
              len(preqs) == 1 and preqs[0]["params"]["sessionId"] == "s1" and "stub-outside" in preqs[0]["params"]["toolCall"]["title"]
              and [o["kind"] for o in preqs[0]["params"]["options"]] == ["allow_once", "allow_always", "reject_once"]
              and psent == ["allow", "allow", "deny"] and pgot and pgot[-1].get("result") == {"stopReason": "end_turn"})
        mcp_out = os.path.join(home, "mcp-answers")
        subprocess.run([coder, "run", "go", "--engine", "claude", "--coder-tools", "--cwd", work, "--json"], cwd=REPO,
                       env=dict(env, PATH=stub + ":" + env["PATH"], CLAUDE_STUB_OUT=mcp_out, CLAUDE_STUB_MODE="mcp"), capture_output=True, text=True, timeout=120)
        got = [json.loads(x)["mcp"] for x in open(mcp_out)] if os.path.exists(mcp_out) else []
        text = got[0]["result"]["content"][0]["text"] if got and got[0] and "result" in got[0] else ""
        check("--coder-tools: Claude Code's call to coder_ls gets the session list", got and got[0].get("id") == 7 and sid[:15] in text)
        sub_out = os.path.join(home, "sub-answers")
        subrun = subprocess.run([coder, "run", "go", "--engine", "claude", "--coder-tools", "--allow", "write", "--ceiling", "write", "--deadline", "60", "--cwd", work, "--json"], cwd=REPO,
                                env=dict(env, PATH=stub + ":" + env["PATH"], CLAUDE_STUB_OUT=sub_out, CLAUDE_STUB_MODE="mcprun"), capture_output=True, text=True, timeout=120)
        psid = next((json.loads(l)["id"] for l in subrun.stdout.splitlines() if '"kind":"session"' in l), "")
        kid = None
        for _ in range(100):
            rows = json.loads(subprocess.run([coder, "ls", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout)
            kid = next((r for r in rows if r.get("task") == "child task"), None)
            if kid and kid["state"] != "running":
                break
            time.sleep(0.2)
        head = {}
        if kid:
            klog = subprocess.run([coder, "log", kid["id"], "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout
            head = next((json.loads(l) for l in klog.splitlines() if '"kind":"session"' in l), {})
        check("a subagent keeps its parent's policy and no more time, and reports back",
              head.get("report_to") == psid and head.get("allow") == "write" and head.get("ceiling") == "write" and 0 < head.get("deadline_s", 0) <= 60)
        hook_home = tempfile.mkdtemp(prefix="coder-hook-"); os.makedirs(os.path.join(hook_home, ".openagents/coder/plugins"))
        with open(os.path.join(hook_home, ".openagents/coder/plugins/guard.json"), "w") as f:
            f.write(json.dumps({"name": "guard", "hooks": [{"on": "tool_call", "run": "case \"$CODER_TOOL\" in *rm*) echo no rm here; exit 1;; esac"}]}))
        hook_out = os.path.join(hook_home, "answers")
        subprocess.run([coder, "run", "go", "--engine", "claude", "--cwd", work, "--json"], cwd=REPO,
                       env=dict(env, HOME=hook_home, PATH=stub + ":" + env["PATH"], CLAUDE_STUB_OUT=hook_out, CLAUDE_STUB_MODE="hook"), capture_output=True, text=True, timeout=120)
        hooked = [json.loads(x) for x in open(hook_out)] if os.path.exists(hook_out) else []
        check("a tool_call hook refuses rm and lets ls run", [(h["command"], h["decision"]) for h in hooked] == [("ls", "allow"), ("rm -rf build", "deny")])
        # Microcoder's commands pass the same hooks and policy before they run (chapter 15)
        mc_home = tempfile.mkdtemp(prefix="coder-mchook-"); os.makedirs(os.path.join(mc_home, ".openagents/coder/plugins"))
        with open(os.path.join(mc_home, ".openagents/coder/plugins/guard.json"), "w") as f:
            f.write(json.dumps({"name": "guard", "hooks": [{"on": "tool_call", "run": "case \"$CODER_TOOL\" in *local-step*) echo no steps here; exit 1;; esac"}]}))
        mh = subprocess.run([coder, "run", "say ok", "--engine", "psionic", "--json"], cwd=REPO, env=dict(env, HOME=mc_home), capture_output=True, text=True, timeout=600)
        ml = [json.loads(l) for l in mh.stdout.splitlines() if l.startswith("{")]
        mhook = [l for l in ml if l["kind"] == "hook"]
        mres = [l for l in ml if l["kind"] == "result"]
        check("microcoder: a tool_call hook refuses its command, and the model is told why",
              len(mhook) == 1 and not mhook[0]["ok"] and mhook[0]["tool"] == "Bash(echo local-step)"
              and len(mres) == 1 and mres[0]["error"] and "no steps here" in mres[0]["text"] and ml[-1].get("ending") == "finished")
        mp_run = subprocess.run([coder, "run", "say ok", "--engine", "psionic", "--ceiling", "write", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=600)
        mpl = [json.loads(l) for l in mp_run.stdout.splitlines() if l.startswith("{")]
        check("microcoder: the policy refuses a command above the ceiling",
              any(l["kind"] == "approval" and l["verdict"] == "refuse" and l["tool"] == "Bash(echo local-step)" for l in mpl)
              and any(l["kind"] == "result" and l["error"] for l in mpl))
        # the agent's terminal (chapter 37): a program that asks a question gets an answer typed into it
        term_out = os.path.join(home, "term-answers")
        calls = [{"name": "terminal_start", "arguments": {"command": "printf 'name? '; read n; echo hi-$n; sleep 5"}},
                 {"name": "terminal_keys", "arguments": {"keys": "bob<Enter>"}},
                 {"name": "terminal_stop", "arguments": {}}]
        subprocess.run([coder, "run", "go", "--engine", "claude", "--coder-tools", "--cwd", work, "--json"], cwd=REPO,
                       env=dict(env, PATH=stub + ":" + env["PATH"], CLAUDE_STUB_OUT=term_out, CLAUDE_STUB_MODE="calls", CLAUDE_STUB_CALLS=json.dumps(calls)),
                       capture_output=True, text=True, timeout=120)
        tr = [json.loads(x)["mcp"]["result"]["content"][0]["text"] for x in open(term_out)] if os.path.exists(term_out) else []
        check("the agent's terminal: a prompt, typed keys, the answer on screen, then stopped",
              len(tr) == 3 and "name?" in tr[0] and "hi-bob" in tr[1] and tr[1].startswith("[running") and tr[2].startswith("[ended"))
        # System One as a tool: coder_decide runs `coder decide`; with no key it says so, and an option with a comma is refused
        dec_out = os.path.join(work, "decide.jsonl")
        calls = [{"name": "coder_decide", "arguments": {"question": "Reply needed?", "options": ["yes", "no"], "state": "thanks!"}},
                 {"name": "coder_decide", "arguments": {"question": "Pick", "options": ["a,b", "c"]}}]
        denv = dict(env, PATH=stub + ":" + env["PATH"], CLAUDE_STUB_OUT=dec_out, CLAUDE_STUB_MODE="calls", CLAUDE_STUB_CALLS=json.dumps(calls))
        denv.pop("BASETEN_API_KEY", None)
        subprocess.run([coder, "run", "go", "--engine", "claude", "--coder-tools", "--cwd", work, "--json"], cwd=REPO, env=denv, capture_output=True, text=True, timeout=120)
        dr = [json.loads(x)["mcp"]["result"] for x in open(dec_out)] if os.path.exists(dec_out) else []
        check("coder_decide: System One as a tool, with no key it says so; an option with a comma is refused",
              len(dr) == 2 and dr[0]["isError"] and "no System One key" in dr[0]["content"][0]["text"]
              and dr[1]["isError"] and "comma" in dr[1]["content"][0]["text"])
        badp = subprocess.run([coder, "run", "hi", "--allow", "run", "--ceiling", "write"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        check("policy: --allow can't pass --ceiling", badp.returncode == 2 and "--allow can't be above --ceiling" in badp.stderr)
        # deadlines and reports (chapter 14): a job past its deadline expires; a job's result reaches the session that started it
        par = subprocess.run([coder, "run", "$ sleep 5", "--engine", "echo", "--detach"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout.strip()
        exp = subprocess.run([coder, "run", "$ sleep 30", "--engine", "echo", "--deadline", "2", "--report-to", par, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        check("--deadline: the job expires, exit 124", exp.returncode == 124 and '"ending":"expired"' in exp.stdout)
        subprocess.run([coder, "run", "child says hi", "--engine", "echo", "--report-to", par], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        for _ in range(100):
            st = subprocess.run([coder, "status", par, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
            if '"running"' not in st.stdout:
                break
            time.sleep(0.2)
        plog = subprocess.run([coder, "log", par, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout
        plines = [json.loads(l) for l in plog.splitlines() if l.startswith("{")]
        jobs = [l["ending"] for l in plines if l["kind"] == "job"]
        check("--report-to: the parent's log holds both jobs' endings", sorted(jobs) == ["expired", "finished"])
        check("--report-to: the running parent takes a report as its next turn", any(l["kind"] == "user" and l["text"].startswith("Job ") and "child says hi" in l["text"] for l in plines))
        # a redirect (chapter 20): ID.turnstop ends the current turn as redirected, and the inbox message is the next turn
        rj = subprocess.run([coder, "run", "$ sleep 20", "--engine", "echo", "--detach"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout.strip()
        sd = os.path.join(home, ".openagents", "coder", "sessions")
        for _ in range(50):
            if os.path.exists(os.path.join(sd, rj + ".jsonl")):
                break
            time.sleep(0.1)
        time.sleep(0.5)
        os.makedirs(os.path.join(sd, rj + ".inbox"), exist_ok=True)
        with open(os.path.join(sd, rj + ".inbox", "1.msg"), "w") as f:
            f.write('{"text":"watchdog says try again"}\n')
        with open(os.path.join(sd, rj + ".turnstop"), "w") as f:
            f.write("redirect\n")
        for _ in range(100):
            st = subprocess.run([coder, "status", rj, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
            if json.loads(st.stdout).get("state") == "finished":
                break
            time.sleep(0.2)
        rl = [json.loads(l) for l in subprocess.run([coder, "log", rj, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout.splitlines() if l.startswith("{")]
        check("a redirect ends the turn as redirected, and the message is the next turn",
              [l["ending"] for l in rl if l["kind"] == "end"] == ["redirected", "finished"] and any(l["kind"] == "user" and l["text"] == "watchdog says try again" for l in rl))
        # coder acp (chapter 16): an editor's session, two prompts in one conversation, and a cancel
        acp = subprocess.Popen([coder, "acp", "--engine", "echo"], cwd=REPO, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
        def acp_send(o):
            acp.stdin.write(json.dumps(o) + "\n"); acp.stdin.flush()
        def acp_until(rid, secs=60):
            got, end = [], time.time() + secs
            while time.time() < end:
                r, _, _ = select.select([acp.stdout], [], [], 0.2)
                if r:
                    line = acp.stdout.readline()
                    if not line:
                        break
                    got.append(json.loads(line))
                    if got[-1].get("id") == rid:
                        break
            return got
        acp_send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1, "clientCapabilities": {}}})
        init = acp_until(1)
        acp_send({"jsonrpc": "2.0", "id": 2, "method": "session/new", "params": {"cwd": REPO, "mcpServers": []}})
        sess = (acp_until(2) or [{}])[-1].get("result", {}).get("sessionId", "")
        acp_send({"jsonrpc": "2.0", "id": 3, "method": "session/prompt", "params": {"sessionId": sess, "prompt": [{"type": "text", "text": "hello acp"}]}})
        g3 = acp_until(3)
        acp_send({"jsonrpc": "2.0", "id": 4, "method": "session/prompt", "params": {"sessionId": sess, "prompt": [{"type": "text", "text": "$ echo second"}]}})
        g4 = acp_until(4)
        acp_send({"jsonrpc": "2.0", "id": 5, "method": "session/prompt", "params": {"sessionId": sess, "prompt": [{"type": "text", "text": "$ sleep 30"}]}})
        time.sleep(2)
        acp_send({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sess}})
        g5 = acp_until(5, 20)
        acp.stdin.close()
        try:
            acp.wait(15)
        except subprocess.TimeoutExpired:
            acp.kill()
        ups = lambda g: [m["params"]["update"] for m in g if m.get("method") == "session/update"]
        check("acp: initialize and session/new", init and init[-1].get("result", {}).get("protocolVersion") == 1 and sess == "s1")
        check("acp: a prompt streams the reply and ends the turn", any(u["sessionUpdate"] == "agent_message_chunk" and "hello acp" in u["content"]["text"] for u in ups(g3))
              and g3[-1].get("result") == {"stopReason": "end_turn"})
        check("acp: a command is a tool call, then its update", [u["sessionUpdate"] for u in ups(g4)] == ["tool_call", "tool_call_update"] and ups(g4)[0]["kind"] == "execute")
        check("acp: session/cancel ends the turn as cancelled", g5 and g5[-1].get("result") == {"stopReason": "cancelled"} and acp.returncode == 0)
        # a progress bar and colors read back as a terminal shows them (chapter 37)
        bar = subprocess.run([coder, "run", "$ printf '10%%\\r100%%\\n\\033[32mok\\033[0m\\n'", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        res = [json.loads(l) for l in bar.stdout.splitlines() if '"kind":"result"' in l]
        check("output reads as the screen shows it", len(res) == 1 and res[0]["text"] == "100%\nok")
        tty = subprocess.run([coder, "run", "$ test -t 1 && echo on-a-terminal; stty size; cat; echo after", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        tres = [json.loads(l) for l in tty.stdout.splitlines() if '"kind":"result"' in l]
        check("a command runs on a terminal the screen's width, with input already ended", len(tres) == 1 and tres[0]["text"] == "on-a-terminal\n24 96\nafter")
        # a plugin's command runs; disabled, nothing it registered remains (chapter 15)
        plugdir = os.path.join(home, ".openagents", "coder", "plugins")
        os.makedirs(plugdir, exist_ok=True)
        with open(os.path.join(plugdir, "demo.json"), "w") as f:
            f.write('{"name":"demo","commands":[{"name":"hello","run":"echo plugin-ok"}]}')
        ran = subprocess.run([coder, "run", "/hello", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        check("a plugin's command runs", any(json.loads(l).get("text") == "plugin-ok" for l in ran.stdout.splitlines() if '"kind":"result"' in l))
        subprocess.run([coder, "run", "/plugin disable demo", "--engine", "echo"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        pl = json.loads(subprocess.run([coder, "plugins", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout)
        check("a disabled plugin keeps no command", pl == [{"name": "demo", "state": "disabled", "signed": False, "commands": [], "defines": [], "provides": [], "uses": []}])
        # a plugin's hooks run on their events, in the session's folder, with the session's context
        with open(os.path.join(plugdir, "hooks.json"), "w") as f:
            f.write('{"name":"hooks","hooks":[{"on":"session_start","run":"echo start-$CODER_EVENT"},{"on":"turn_end","run":"echo end-$CODER_ENDING-$CODER_ENGINE"}]}')
        hk = subprocess.run([coder, "run", "hi", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        hl = [json.loads(l) for l in hk.stdout.splitlines() if '"kind":"hook"' in l]
        check("a plugin's hooks run on session_start and turn_end", [(h["on"], h["text"]) for h in hl] == [("session_start", "start-session_start"), ("turn_end", "end-finished-echo")])
        os.remove(os.path.join(plugdir, "hooks.json"))
        # capabilities and profiles: a user's command gets its provider's command; a profile loads its plugins in order
        with open(os.path.join(plugdir, "fmt.json"), "w") as f:
            f.write('{"name":"fmt","defines":[{"name":"format"}],"provides":[{"name":"format","run":"echo formatted"}]}')
        with open(os.path.join(plugdir, "tidy.json"), "w") as f:
            f.write('{"name":"tidy","uses":["format"],"commands":[{"name":"tidy","run":"sh -c \\"$CODER_CAP_FORMAT\\""}]}')
        td = subprocess.run([coder, "run", "/tidy", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        check("capabilities: a user's command runs its provider's command", any(json.loads(l).get("text") == "formatted" for l in td.stdout.splitlines() if '"kind":"result"' in l))
        os.makedirs(os.path.join(home, ".openagents", "coder", "profiles"), exist_ok=True)
        with open(os.path.join(home, ".openagents", "coder", "profiles", "lean.json"), "w") as f:
            f.write('{"plugins":["tidy","fmt"]}')
        lean = json.loads(subprocess.run([coder, "plugins", "--json", "--profile", "lean"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout)
        check("profiles: only the named plugins, in order; a user before its provider fails",
              [(x["name"], x["state"]) for x in lean] == [("tidy", "failed"), ("fmt", "active")] and lean[0]["why"] == "uses format, which no active plugin provides")
        nop = subprocess.run([coder, "plugins", "--profile", "nope"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        check("profiles: an unknown profile is refused", nop.returncode == 2 and "no such profile" in nop.stderr)
        os.remove(os.path.join(plugdir, "fmt.json")); os.remove(os.path.join(plugdir, "tidy.json"))
        # --on ssh:DEST (chapter 23): a stand-in ssh runs the remote line here, in another HOME whose openagents is this checkout
        rhome = tempfile.mkdtemp(prefix="coder-remote-"); os.symlink(REPO, os.path.join(rhome, "openagents"))
        with open(os.path.join(fake, "ssh"), "w") as f:
            f.write("#!/bin/sh\nfor a; do last=\"$a\"; done\nHOME=" + rhome + " exec sh -c \"$last\"\n")
        os.chmod(os.path.join(fake, "ssh"), 0o755)
        senv = dict(env, PATH=fake + ":" + env["PATH"])
        rr = subprocess.run([coder, "run", "$ echo remote-ok", "--engine", "echo", "--json", "--on", "ssh:box"], cwd=REPO, env=senv, capture_output=True, text=True, timeout=600)
        rl = [json.loads(l) for l in rr.stdout.splitlines() if l.startswith("{")]
        rid = rl[0].get("id", "") if rl and rl[0].get("kind") == "session" else ""
        recs = [n for n in os.listdir(os.path.join(home, ".openagents", "coder", "sessions")) if n.endswith(".remote")]
        rs = subprocess.run([coder, "status", rid[:15] or "none", "--json"], cwd=REPO, env=senv, capture_output=True, text=True, timeout=120)
        check("--on ssh: the run happens on the other machine, is recorded here, and status by prefix is forwarded",
              rr.returncode == 0 and any(l.get("text") == "remote-ok" for l in rl) and recs == [rid + ".remote"]
              and any(n.startswith(rid) for n in os.listdir(os.path.join(rhome, ".openagents", "coder", "sessions"))) and rid in rs.stdout and "finished" in rs.stdout)
        # replay (chapter 9): a Microcoder run recorded from Vertex replays through the loop with no credentials, the same each time
        import re, shutil
        fx = os.path.join(HERE, "fixtures", "micro-replay")
        rw = tempfile.mkdtemp(prefix="coder-replay-")
        renv = dict(env, OPENAGENTS_SECRETS="/nonexistent")
        for k in ("VERTEX_ACCESS_TOKEN", "GOOGLE_APPLICATION_CREDENTIALS", "GEMINI_API_KEY", "OPENROUTER_API_KEY"):
            renv.pop(k, None)
        def replay():
            rd = tempfile.mkdtemp(prefix="coder-replay-dir-")
            for n in ("1.http", "2.http"):
                shutil.copy(os.path.join(fx, n), rd)
            r = subprocess.run([coder, "run", "Run `echo replay-check` and then tell me what it printed, in one short sentence.", "--engine", "micro", "--json", "--cwd", rw],
                               cwd=REPO, env=dict(renv, GW_REPLAY=rd), capture_output=True, text=True, timeout=300)
            return r.returncode, re.sub(r'"(id|cwd)":"[^"]*"|"(ms|started_us)":[0-9]*', "", r.stdout)
        ra, rb = replay(), replay()
        check("replay: recorded gateway streams run through the loop with no credentials, byte-identical each time",
              ra[0] == 0 and ra == rb and '"text":"It printed replay-check."' in ra[1] and '"label":"Bash(echo replay-check)"' in ra[1])
        # memory (chapter 19): remember, a conflict, a revise that settles it, forget; recall ranks by replayed embeddings
        mhome = tempfile.mkdtemp(prefix="coder-memory-")
        menv = dict(renv, HOME=mhome)
        def mq(*a, e=None):
            return subprocess.run([coder] + list(a), cwd=rw, env=e or menv, capture_output=True, text=True, timeout=300)
        m1 = mq("remember", "deploy", "Ship openagents.com only through coax/web/release.sh.", "--project")
        m2 = mq("remember", "build", "Build Coder with make.")
        m3 = mq("remember", "build", "Build with the old script.")
        m4 = mq("revise", "build", "Build with coder/tui/coder.sh.", "--reason", "resolve")
        m5 = mq("remember", "cake", "The office cake is lemon on Fridays.")
        m6 = mq("forget", "cake")
        mdir = os.path.join(mhome, ".openagents", "coder", "memory")
        evs = open(os.path.join(mdir, "events.jsonl")).read()
        mfx = tempfile.mkdtemp(prefix="coder-memory-replay-")
        for n in ("1.http", "2.http", "3.http"):
            shutil.copy(os.path.join(HERE, "fixtures", "memory-replay", n), mfx)
        mr = mq("recall", "how do I put the website live?", "--json", e=dict(menv, GW_REPLAY=mfx))
        mj = json.loads(mr.stdout or "{}")
        vlines = open(os.path.join(mdir, "vectors.jsonl")).read().count("\n")
        m7 = mq("forget", "build")
        vafter = open(os.path.join(mdir, "vectors.jsonl")).read().count("\n")
        # the first message of a session carries the remembered facts, logged so replay rebuilds it; --no-memory leaves them out
        mb = mq("run", "hi", "--engine", "echo", "--json")
        mbl = [json.loads(l) for l in mb.stdout.splitlines() if l.startswith("{")]
        mbid = next((l["id"] for l in mbl if l.get("kind") == "session"), "")
        mbr = mq("replay", mbid)
        mbn = mq("run", "hi", "--engine", "echo", "--no-memory", "--json")
        check("memory: a session's first message carries the remembered facts, and replay rebuilds it",
              any(l.get("kind") == "memory" and l.get("text") == "Facts I asked you to remember:\n- deploy: Ship openagents.com only through coax/web/release.sh." for l in mbl)
              and "You said: Facts I asked you to remember:" in mb.stdout and "1 of 1 requests rebuilt with the same bytes." in mbr.stdout
              and '"kind":"memory"' not in mbn.stdout and "You said: hi" in mbn.stdout)
        # consent (chapter 18): facts reach an engine only under a grant for it and their class; the ledger counts what went
        def gq(*a):
            gd = tempfile.mkdtemp(prefix="coder-gate-")
            for n in ("1.http", "2.http"):
                shutil.copy(os.path.join(fx, n), gd)
            r = mq("run", "Run `echo replay-check` and then tell me what it printed, in one short sentence.", "--engine", "micro", "--json", e=dict(menv, GW_REPLAY=gd))
            return next((json.loads(l) for l in r.stdout.splitlines() if '"kind":"memory"' in l), {})
        g0 = gq()
        gu = mq("consent", "grant", "micro", "--classes", "user")
        g1 = gq()
        gp = mq("consent", "grant", "micro", "--classes", "project")
        g2 = gq()
        gr = mq("consent", "revoke", "1")
        g3 = gq()
        gj = json.loads(mq("consent", "--json").stdout or "{}")
        check("consent: remembered facts reach an engine only under a grant for their class; a revoke stops them; the ledger replays",
              g0 == {"kind": "memory", "text": "", "withheld": 1} and g1.get("withheld") == 1 and gu.returncode == 0 and gp.returncode == 0 and gr.returncode == 0
              and g2.get("text") == "Facts I asked you to remember:\n- deploy: Ship openagents.com only through coax/web/release.sh." and "withheld" not in g2
              and g3.get("withheld") == 1 and gj.get("events") == 7
              and [(x["engine"], x["classes"], x["state"], x["sends"]) for x in gj.get("grants", [])] == [("microcoder", "user", "active", 1), ("microcoder", "project", "revoked", 1)])
        open(os.path.join(mdir, "events.jsonl"), "w").write(evs.replace('"from":0', '"from":5', 1))
        m8 = mq("recall")
        check("memory: remember, conflict, revise, forget; recall ranks by embeddings; forgotten text leaves the file; an edited log is refused",
              all(x.returncode == 0 for x in (m1, m2, m4, m5, m6, m7)) and "now holds two texts" in m3.stdout and "version 3" in m4.stdout
              and "lemon" not in evs and len(evs.splitlines()) == 6
              and mr.returncode == 0 and mj.get("ranked") is True and [f["key"] for f in mj.get("facts", [])] == ["deploy", "build"]
              and mj["facts"][0]["score"] > mj["facts"][1]["score"] and mj["facts"][1]["version"] == 3
              and vlines == 2 and vafter == 1 and m8.returncode == 1 and "does not verify" in m8.stderr)
        # each Microcoder ending through coder run (chapter 9), from replayed responses
        def mhttp(content, status=200):
            if status != 200:
                return "HTTP/2 %d \r\ncontent-type: application/json\r\n\r\n{\"error\":{\"code\":%d,\"message\":\"overloaded\"}}\n" % (status, status)
            chunk = {"choices": [{"delta": {"content": content, "role": "assistant"}, "index": 0}], "object": "chat.completion.chunk"}
            last = {"choices": [{"delta": {"content": ""}, "finish_reason": "stop", "index": 0}], "object": "chat.completion.chunk",
                    "usage": {"prompt_tokens": 500, "completion_tokens": 40, "total_tokens": 540}}
            return "HTTP/2 200 \r\ncontent-type: text/event-stream\r\n\r\ndata: " + json.dumps(chunk) + "\n\ndata: " + json.dumps(last) + "\n\ndata: [DONE]\n"
        act = lambda **k: json.dumps(dict({"rationale": "r", "commands": [], "view": [], "finished": False, "reply": "", "ask": "none"}, **k))
        endings = {}
        for name, rs in {"asked": [mhttp(act(reply="Which file?", ask="question"))], "bad_replies": [mhttp("not json")] * 3,
                         "no_capacity": [mhttp("", 503)] * 4, "idle": [mhttp(act())] * 3}.items():
            ed = tempfile.mkdtemp(prefix="coder-ending-")
            for i, r in enumerate(rs):
                with open(os.path.join(ed, "%d.http" % (i + 1)), "w") as f:
                    f.write(r)
            ep = subprocess.run([coder, "run", "do it", "--engine", "micro", "--json", "--cwd", ed], cwd=REPO, env=dict(renv, GW_REPLAY=ed), capture_output=True, text=True, timeout=300)
            el = [json.loads(l) for l in ep.stdout.splitlines() if l.startswith("{")]
            endings[name] = (ep.returncode, el[-1].get("ending") if el else "", el[-1].get("text", "") if el else "", [l["text"] for l in el if l["kind"] == "reply"])
        check("microcoder endings through coder run: asked, bad_replies, no_capacity and idle",
              endings["asked"] == (0, "finished", "", ["Which file?"])
              and endings["bad_replies"][:3] == (1, "failed", "The model's replies didn't fit Microcoder's schema three times in a row.")
              and endings["no_capacity"][:3] == (1, "failed", "No model answered: overloaded")
              and endings["idle"][:3] == (1, "failed", "The model gave nothing to do and no reply three times in a row."))
        # goals (chapter 14): the check runs after each finished turn; its output is the next turn until it passes
        gw = tempfile.mkdtemp(prefix="coder-goal-")
        gm = subprocess.run([coder, "run", "$ echo working", "--engine", "echo", "--json", "--cwd", gw, "--goal", "test -f c || { touch c; echo c-missing; false; }"],
                            cwd=REPO, env=env, capture_output=True, text=True, timeout=300)
        gl = [json.loads(l) for l in gm.stdout.splitlines() if l.startswith("{")]
        gu = subprocess.run([coder, "run", "$ echo trying", "--engine", "echo", "--json", "--cwd", gw, "--goal", "false", "--goal-turns", "2"],
                            cwd=REPO, env=env, capture_output=True, text=True, timeout=300)
        gx = [json.loads(l) for l in gu.stdout.splitlines() if l.startswith("{")]
        check("goals: a failing check's output is the next turn until it passes; a cap the person sets ends it unmet",
              gm.returncode == 0 and [(l["turn"], l["met"]) for l in gl if l["kind"] == "goal"] == [(1, False), (2, True)]
              and any(l["kind"] == "user" and "c-missing" in l["text"] for l in gl)
              and gu.returncode == 1 and [(l["turn"], l["met"]) for l in gx if l["kind"] == "goal"] == [(1, False), (2, False)])
        # coder daemon (chapters 20 and 23): queued jobs start within the grant; a job for a machine that does not answer is held, then sent
        dq = lambda *a, e=env: subprocess.run([coder, *a], cwd=REPO, env=e, capture_output=True, text=True, timeout=300)
        dq("queue", "$ echo daemon-ok", "--engine", "echo", "--cwd", home); dq("queue", "x", "--engine", "claude", "--cwd", home)
        dq("queue", "y", "--engine", "echo", "--cwd", "/etc")
        dq("daemon", "--once", "--engines", "echo,micro")
        dj = json.loads(dq("jobs", "--json").stdout)
        dst = sorted((j["state"], j.get("why", "")) for j in dj["jobs"])
        check("daemon: a queued job starts as a session; an engine or directory outside the grant is refused",
              dj["daemon"]["engines"] == "echo,micro" and dst == [("refused", "its directory is outside this machine's grant"), ("refused", "its engine is outside this machine's grant"), ("started", "")])
        with open(os.path.join(fake, "ssh"), "w") as f:
            f.write("#!/bin/sh\nexit 255\n")
        hq = dq("queue", "$ echo held-ok", "--engine", "echo", "--cwd", rhome, "--on", "ssh:box", e=senv)
        held = [j for j in json.loads(dq("jobs", "--json").stdout)["jobs"] if j["state"] == "held"]
        with open(os.path.join(fake, "ssh"), "w") as f:
            f.write("#!/bin/sh\nfor a; do last=\"$a\"; done\nHOME=" + rhome + " exec sh -c \"$last\"\n")
        dq("daemon", "--once", e=senv)
        sent = [j for j in json.loads(dq("jobs", "--json").stdout)["jobs"] if j["state"] == "sent"]
        dq("daemon", "--once", e=dict(env, HOME=rhome))
        rj = [j for j in json.loads(dq("jobs", "--json", e=dict(env, HOME=rhome)).stdout)["jobs"] if j["task"] == "$ echo held-ok"]
        check("daemon: a job for a machine that does not answer is held, then sent when it does, and starts there",
              hq.returncode == 0 and len(held) == 1 and held[0]["dest"] == "box" and len(sent) == 1 and sent[0].get("remote_job")
              and len(rj) == 1 and rj[0]["state"] == "started" and rj[0]["job"] == sent[0]["remote_job"])
        os.remove(os.path.join(fake, "ssh"))
        # credential handles (chapter 20): a job names a handle; only a grant that lists it gets the variable, and the log hides the value
        cv = "ghp_SMOKEVALUE1234567890"
        ca = subprocess.run([coder, "credential", "add", "gh", "GH_TOKEN"], cwd=REPO, env=env, input=cv + "\n", capture_output=True, text=True, timeout=60)
        cmode = oct(os.stat(os.path.join(home, ".openagents/coder/credentials/gh.env")).st_mode & 0o777)
        cls = dq("credential", "ls", "--json").stdout
        dq("queue", "$ printenv GH_TOKEN | wc -c; printenv GH_TOKEN", "--engine", "echo", "--cwd", home, "--credential", "gh")
        dq("daemon", "--once", "--engines", "echo")
        cr1 = [j for j in json.loads(dq("jobs", "--json").stdout)["jobs"] if j["task"].startswith("$ printenv GH_TOKEN")]
        dq("queue", "$ printenv GH_TOKEN | wc -c; printenv GH_TOKEN", "--engine", "echo", "--cwd", home, "--credential", "gh")
        dq("daemon", "--once", "--engines", "echo", "--credentials", "gh")
        cr2 = [j for j in json.loads(dq("jobs", "--json").stdout)["jobs"] if j["task"].startswith("$ printenv GH_TOKEN") and j["state"] == "started"]
        cfile = os.path.join(home, ".openagents/coder/sessions", (cr2[0].get("session", "") if cr2 else "none") + ".jsonl")
        clog = ""
        for _ in range(60):
            clog = open(cfile).read() if os.path.exists(cfile) else ""
            if '"kind":"end"' in clog: break
            time.sleep(0.5)
        check("credentials: a job gets a handle's variable only under a grant that lists it, and its log shows [credential]",
              ca.returncode == 0 and cmode == "0o600" and json.loads(cls) == [{"name": "gh", "var": "GH_TOKEN"}]
              and len(cr1) == 1 and cr1[0].get("why") == "a credential it names is outside this machine's grant"
              and len(cr2) == 1 and "[credential]" in clog and str(len(cv) + 1) in clog and cv not in clog)
        # Wasm plugins: built from Coax, run in the bounded host; a module importing more than it was admitted is refused
        if os.path.exists("/opt/homebrew/opt/llvm/bin/clang") and os.path.isdir("/opt/homebrew/opt/wasi-libc/share/wasi-sysroot"):
            wsrc = os.path.join(REPO, "coder", "plugins", "hello", "hello.coax"); wsh = os.path.join(HERE, "wasm_plugin.sh")
            subprocess.run([wsh, wsrc, os.path.join(plugdir, "hello.wasm"), "hello", "spin"], check=True, capture_output=True, timeout=300)
            subprocess.run([wsh, wsrc, os.path.join(plugdir, "clocky.wasm"), "hello", "clock"], check=True, capture_output=True, timeout=300)
            with open(os.path.join(plugdir, "hello.json"), "w") as f:
                f.write(open(os.path.join(REPO, "coder", "plugins", "hello", "hello.json")).read())
            with open(os.path.join(plugdir, "clocky.json"), "w") as f:
                f.write('{"name":"clocky","wasm":{"module":"clocky.wasm","imports":["coder_print"]},"commands":[{"name":"clock","export":"clock"}]}')
            wp = {x["name"]: x for x in json.loads(subprocess.run([coder, "plugins", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout)}
            wh = subprocess.run([coder, "run", "/hello", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
            ws = subprocess.run([coder, "run", "/spin", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
            whr = [json.loads(l) for l in wh.stdout.splitlines() if '"kind":"result"' in l]
            wsr = [json.loads(l) for l in ws.stdout.splitlines() if '"kind":"result"' in l]
            check("wasm plugins: a command runs in the host; one that never returns is stopped; an unadmitted import is refused",
                  wp.get("hello", {}).get("state") == "active" and wp.get("clocky", {}).get("why") == "it imports env.coder_now_ms, which it was not admitted"
                  and whr and whr[0]["text"] == "hello from wasm" and wsr and wsr[0]["error"] and "stopped after 3s" in wsr[0]["text"])
            for n in ("hello.wasm", "hello.json", "clocky.wasm", "clocky.json"):
                os.remove(os.path.join(plugdir, n))
        else:
            print("skip wasm plugins: no LLVM with a wasm32 target and wasi-libc")
        # fork: the new session carries the parent's turns, and its header names the parent
        first = subprocess.run([coder, "run", "remember heliotrope", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        pid_ = json.loads(first.stdout.splitlines()[0])["id"]
        fork = subprocess.run([coder, "fork", pid_, "what word?", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        fl = [json.loads(l) for l in fork.stdout.splitlines() if l.startswith("{")]
        check("fork carries the parent's turns", fork.returncode == 0 and fl[0].get("parent") == pid_ and fl[0].get("at") == 1
              and any("User: remember heliotrope" in l.get("text", "") and "Now: what word?" in l.get("text", "") for l in fl if l["kind"] == "reply"))
        # replay rebuilds every request from the log, a fork's from its parent's too, and finds a changed log
        fid = fl[0]["id"] if fl else "none"
        rp = subprocess.run([coder, "replay", fid, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        fpath = os.path.join(home, ".openagents", "coder", "sessions", fid + ".jsonl")
        body = open(fpath).read() if os.path.exists(fpath) else ""
        with open(fpath, "w") as f:
            f.write(body.replace('"text":"what word?"', '"text":"what WORD?"'))
        rp2 = subprocess.run([coder, "replay", fid, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        with open(fpath, "w") as f:
            f.write(body)
        check("replay: a fork's request rebuilds with the same bytes; a changed log does not",
              rp.returncode == 0 and json.loads(rp.stdout)["matched"] == 1 and rp2.returncode == 1 and json.loads(rp2.stdout)["matched"] == 0)
        # attach: a message becomes a running job's next turn; to an ended job, it continues in a fork
        aj = subprocess.run([coder, "run", "$ sleep 2; echo one", "--engine", "echo", "--detach"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120).stdout.strip()
        at1 = subprocess.run([coder, "attach", aj, "$ echo two"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        at2 = subprocess.run([coder, "attach", aj, "$ echo three"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        check("attach: the job takes the message as its next turn", at1.returncode == 0 and "› $ echo two" in at1.stdout and "    └ two" in at1.stdout)
        check("attach: an ended job's conversation continues in a fork", at2.returncode == 0 and "continuing in" in at2.stdout and "    └ three" in at2.stdout)
        # a background job, then stop it
        job = subprocess.run([coder, "run", "$ sleep 60", "--engine", "echo", "--detach"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        jid = job.stdout.strip()
        running = False
        for _ in range(100):
            st = subprocess.run([coder, "status", jid, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
            if st.returncode == 0 and json.loads(st.stdout)["state"] == "running":
                running = True
                break
            time.sleep(0.2)
        check("run --detach returns an ID and the job runs", job.returncode == 0 and running)
        stop = subprocess.run([coder, "stop", jid], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        st = subprocess.run([coder, "status", jid, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        check("stop ends the job as stopped", stop.returncode == 0 and json.loads(st.stdout)["state"] == "stopped")
        # the Router (chapters 17 and 20): an event addressed to the agent starts a Worker, a later event on its
        # thread goes to that Worker as a fork, an ignored source is dropped, and a Worker whose process dies is lost
        rdir = os.path.join(home, ".openagents", "coder", "router")
        os.makedirs(rdir, exist_ok=True)
        with open(os.path.join(rdir, "sources.json"), "w") as f:
            f.write('{"sources":{"local":{"mode":"explicit_requests_only","authors":["me"],"engine":"echo"},"spam":{"mode":"ignore","authors":["*"]}}}')
        def router():
            r = subprocess.run([coder, "router", "--once", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=300)
            return [json.loads(l) for l in r.stdout.splitlines() if l.startswith("{")]
        def trigger(*a):
            return subprocess.run([coder, "trigger", *a], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).returncode
        ok = trigger("$ echo first", "--thread", "ci", "--to-agent") == 0 and trigger("buy now", "--source", "spam", "--to-agent") == 0 \
            and trigger("just chatting", "--thread", "lunch") == 0
        r1 = router()
        fates = sorted(l["fate"] for l in r1)
        check("router: a request starts a worker; ignored and unaddressed events drop", ok and fates == sorted([
            "addressed to the agent: new worker", "dropped: source ignored", "dropped: not addressed to the agent"]))
        w1 = next((l.get("session", "") for l in r1 if l.get("thread") == "ci"), "")
        for _ in range(100):
            st = subprocess.run([coder, "status", w1, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
            if st.returncode == 0 and json.loads(st.stdout)["state"] == "finished":
                break
            time.sleep(0.2)
        r1b = router()
        outbox = os.path.join(home, ".openagents", "coder", "outbox")
        reply = json.load(open(os.path.join(outbox, w1 + ".json"))) if os.path.exists(os.path.join(outbox, w1 + ".json")) else {}
        check("router: a finished worker's reply goes to the outbox under the chat rules", any(l["fate"] == "reply written to the outbox" for l in r1b)
              and reply.get("channel") == "chat" and reply.get("text") == "first")
        trigger("$ sleep 60", "--thread", "ci")
        r2 = router()
        w2 = r2[0].get("session", "") if r2 else ""
        for _ in range(100):                # the Worker writes its session file a moment after it starts
            head = subprocess.run([coder, "log", w2, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
            if head.returncode == 0 and head.stdout.strip():
                break
            time.sleep(0.2)
        check("router: a later event on the thread goes to its worker", len(r2) == 1 and r2[0]["fate"] == "passed to its thread's worker"
              and w2 != w1 and head.returncode == 0 and json.loads(head.stdout.splitlines()[0]).get("parent") == w1)
        wpid = 0
        for _ in range(100):
            try:
                wpid = int(open(os.path.join(home, ".openagents", "coder", "sessions", w2 + ".pid")).read().strip())
                break
            except (OSError, ValueError):
                time.sleep(0.2)
        if wpid:
            os.kill(wpid, 9)                # the Worker this smoke started
            time.sleep(0.5)
        r3 = router()
        st = subprocess.run([coder, "status", w2, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        check("watchdog: a worker whose process died ends as lost", any(l["fate"] == "worker lost: ended" for l in r3)
              and st.returncode == 0 and json.loads(st.stdout)["state"] == "lost")
        # batching (chapter 17): a pass's candidates go to System One in one request, at most 8; the rest wait for the next pass
        with open(os.path.join(rdir, "sources.json"), "w") as f:
            f.write('{"sources":{"local":{"mode":"explicit_requests_only","authors":["me"],"engine":"echo"},"spam":{"mode":"ignore","authors":["*"]},'
                    '"team":{"mode":"candidate","authors":["*"],"engine":"echo","policy":"work on requests for changes"}}}')
        for i in range(10):
            trigger("message %d" % i, "--source", "team", "--author", "kim")
        nk = dict(env); nk.pop("BASETEN_API_KEY", None)
        def rpass():
            r = subprocess.run([coder, "router", "--once", "--json"], cwd=REPO, env=nk, capture_output=True, text=True, timeout=300)
            return [json.loads(l) for l in r.stdout.splitlines() if l.startswith("{")]
        b1, b2 = rpass(), rpass()
        def counts(ls):
            return ([l["events"] for l in ls if l.get("kind") == "batch"], sum(1 for l in ls if l.get("fate") == "dropped: no decision"))
        check("router batching: eight candidates in one System One request, the other two in the next pass",
              counts(b1) == ([8], 8) and counts(b2) == ([2], 2) and all(not l["decided"] for l in b1 + b2 if l.get("kind") == "batch"))
        # the watchdog: a worker that repeats one step is redirected, then stopped; a quiet one with no System One key is reported
        sdir = os.path.join(home, ".openagents", "coder", "sessions")
        nokey = dict(env)
        nokey.pop("BASETEN_API_KEY", None)
        sleeper = subprocess.Popen(["sleep", "60"])
        try:
            def fake(sid, lines, old):
                with open(os.path.join(sdir, sid + ".jsonl"), "w") as f:
                    f.write('{"kind":"session","id":"%s","engine":"echo","cwd":"/","started_us":1}\n{"kind":"user","text":"fix the build"}\n' % sid + lines)
                with open(os.path.join(sdir, sid + ".pid"), "w") as f:
                    f.write(str(sleeper.pid))
                if old:
                    os.utime(os.path.join(sdir, sid + ".jsonl"), (time.time() - 600, time.time() - 600))
            fake("20261102-000000-001", '{"kind":"tool","label":"Bash(make)"}\n' * 5, False)
            fake("20261102-000000-002", '{"kind":"tool","label":"Bash(ls)"}\n', True)
            with open(os.path.join(rdir, "threads"), "a") as f:
                f.write("loop\t20261102-000000-001\tlocal\nquiet\t20261102-000000-002\tlocal\n")
            r = subprocess.run([coder, "router", "--once", "--json", "--stuck", "60"], cwd=REPO, env=nokey, capture_output=True, text=True, timeout=120)
            rw = [json.loads(l) for l in r.stdout.splitlines() if l.startswith("{")]
            check("watchdog: a worker repeating one step is redirected first", any(l["fate"] == "worker redirected: looping" and l.get("session") == "20261102-000000-001" for l in rw)
                  and os.path.exists(os.path.join(sdir, "20261102-000000-001.turnstop")) and not os.path.exists(os.path.join(sdir, "20261102-000000-001.stop"))
                  and any(n.endswith(".msg") for n in os.listdir(os.path.join(sdir, "20261102-000000-001.inbox"))))
            r = subprocess.run([coder, "router", "--once", "--json", "--stuck", "60"], cwd=REPO, env=nokey, capture_output=True, text=True, timeout=120)
            rw2 = [json.loads(l) for l in r.stdout.splitlines() if l.startswith("{")]
            check("watchdog: still repeating after a redirect, it is stopped", any(l["fate"] == "worker stopped: looping" and l.get("session") == "20261102-000000-001" for l in rw2)
                  and os.path.exists(os.path.join(sdir, "20261102-000000-001.stop")))
            check("watchdog: a quiet worker with no decision is reported as stuck", any(l["fate"] == "worker stuck" and "System One key" in l.get("note", "") for l in rw))
        finally:
            sleeper.kill()
        # duplex (chapter 20): a follow-up gets its reply while the Thread's long Worker runs; the long result comes later; a stopped one says so
        trigger("$ sleep 8; echo long-done", "--thread", "dx", "--to-agent")
        dl = next((l.get("session", "") for l in router() if l.get("thread") == "dx"), "")
        time.sleep(1.0)
        trigger("$ echo quick-dx", "--thread", "dx", "--to-agent")
        d0 = time.time(); dquick = None; dlong = None; drun = False; dlog = []
        for _ in range(80):
            ls = router(); dlog += ls
            for l in ls:
                if l.get("thread") == "dx" and l.get("fate") == "reply written to the outbox":
                    if l.get("note") == "quick-dx" and dquick is None:
                        dquick = time.time() - d0
                        drun = json.loads(subprocess.run([coder, "status", dl, "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60).stdout)["state"] == "running"
                    if l.get("note") == "long-done": dlong = time.time() - d0
            if dquick is not None and dlong is not None: break
            time.sleep(0.5)
        trigger("$ sleep 60", "--thread", "dy", "--to-agent")
        ds = next((l.get("session", "") for l in router() if l.get("thread") == "dy"), "")
        time.sleep(1.0)
        subprocess.run([coder, "stop", ds], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        dstop = [l for l in router() if l.get("thread") == "dy" and l.get("fate") == "reply written to the outbox"]
        check("duplex: a follow-up is answered while the Thread's long Worker runs, the long result arrives later, and a stopped Worker says so",
              dl and dquick is not None and drun and dlong is not None and dlong > dquick
              and len(dstop) == 1 and dstop[0].get("note", "").startswith("Stopped (stopped)"))
        if dquick is not None: print("     duplex: follow-up answered in %.1f s with the long Worker running; long result after %.1f s" % (dquick, dlong or -1))
        # a log cut at any byte (chapter 2): readers read the complete lines and report the rest; a line appended after a crash starts clean
        cr = subprocess.run([coder, "run", "$ echo cut-é-check", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        cid = next(json.loads(l)["id"] for l in cr.stdout.splitlines() if l.startswith('{"kind":"session"'))
        cpath = os.path.join(sdir, cid + ".jsonl"); cdata = open(cpath, "rb").read()
        cbad = []
        for cut in range(0, len(cdata) + 1):
            open(cpath, "wb").write(cdata[:cut])
            whole = cdata[:cut].rfind(b"\n") + 1
            st = subprocess.run([coder, "status", cid, "--json"], cwd=REPO, env=env, capture_output=True, timeout=60)
            lg = subprocess.run([coder, "log", cid, "--json"], cwd=REPO, env=env, capture_output=True, timeout=60)
            if whole == 0:
                if st.returncode != 6: cbad.append((cut, "status", st.returncode))
                continue
            try:
                sj = json.loads(st.stdout)
                ok = st.returncode == 0 and sj.get("cut", 0) == cut - whole and lg.returncode == 0 and lg.stdout == cdata[:whole] \
                     and all(json.loads(l) for l in lg.stdout.decode().splitlines())
            except Exception as e:
                ok = False
            if not ok: cbad.append((cut, st.stdout[:80], lg.returncode))
        mid = cdata.rfind(b"\n", 0, len(cdata) - 1) + 10          # inside the end line
        open(cpath, "wb").write(cdata[:mid])
        with open(os.path.join(sdir, cid + ".pid"), "w") as f:
            f.write("999999")
        with open(os.path.join(rdir, "threads"), "a") as f:
            f.write("cutlog\t%s\tlocal\n" % cid)
        subprocess.run([coder, "router", "--once", "--json"], cwd=REPO, env=nokey, capture_output=True, timeout=120)
        after = open(cpath, "rb").read()
        check("a log cut at any byte: status and log read the complete lines and report the rest; the lost line after a crash starts on its own line",
              not cbad and after.startswith(cdata[:cdata.rfind(b"\n", 0, len(cdata) - 1) + 1])
              and json.loads(after.decode().splitlines()[-1])["ending"] == "lost"
              and open(cpath + ".cut", "rb").read() == cdata[cdata.rfind(b"\n", 0, len(cdata) - 1) + 1:mid] + b"\n")
        if cbad: print("     cut failures:", cbad[:5])
        # rechecks (chapter 20): a Worker whose model provider did not answer waits, then continues in a fork the Thread follows
        fsid = "20261102-000000-003"
        with open(os.path.join(sdir, fsid + ".jsonl"), "w") as f:
            f.write('{"kind":"session","id":"%s","engine":"echo","cwd":"/","started_us":1}\n{"kind":"user","text":"fix the build"}\n'
                    '{"kind":"end","ending":"failed","ms":5,"steps":0,"text":"No model answered: overloaded","cause":"provider"}\n' % fsid)
        with open(os.path.join(rdir, "threads"), "a") as f:
            f.write("flaky\t%s\tlocal\n" % fsid)
        def rpass2(first):
            r = subprocess.run([coder, "router", "--once", "--json"], cwd=REPO, env=dict(nokey, CODER_ROUTER_RECHECK_S=first), capture_output=True, text=True, timeout=120)
            return [json.loads(l) for l in r.stdout.splitlines() if l.startswith("{")]
        rc1, rc2 = rpass2("600"), rpass2("0")
        rk = [l for l in rc2 if l.get("fate") == "worker rechecked: the model provider did not answer"]
        nsid = rk[0]["session"] if rk else ""
        for _ in range(60):
            if nsid and '"kind":"end"' in (open(os.path.join(sdir, nsid + ".jsonl")).read() if os.path.exists(os.path.join(sdir, nsid + ".jsonl")) else ""): break
            time.sleep(0.5)
        rc3 = rpass2("0")
        tmap = dict(l.split("\t")[:2] for l in open(os.path.join(rdir, "threads")).read().splitlines() if "\t" in l)
        check("rechecks: a Worker the model provider failed waits, then a fork continues it and the Thread follows; the failure is not reported",
              any(l.get("fate") == "worker waiting: the model provider did not answer" and l.get("session") == fsid for l in rc1)
              and len(rk) == 1 and tmap.get("flaky") == nsid and nsid != fsid
              and not any(l.get("session") == fsid and "reply" in l.get("fate", "") for l in rc1 + rc2 + rc3)
              and any(l.get("session") == nsid for l in rc3))
        # a GitHub source, against a stand-in gh: the first poll sets the cursor; a labelled issue starts a worker; its
        # reply is posted as a comment; the Router's own comment does not come back as an event
        stub = os.path.join(home, "gh")
        with open(stub, "w") as f:
            f.write("#!" + sys.executable + "\n" + GH_STUB)
        os.chmod(stub, 0o755)
        genv = dict(env, CODER_GH=stub, GH_STUB_DIR=home)
        with open(os.path.join(rdir, "sources.json"), "w") as f:
            f.write('{"sources":{"github:o/r":{"mode":"explicit_requests_only","authors":["alice"],"label":"coder","engine":"echo","deliver":true}}}')
        def grouter():
            r = subprocess.run([coder, "router", "--once", "--json"], cwd=REPO, env=genv, capture_output=True, text=True, timeout=300)
            return [l for l in (json.loads(x) for x in r.stdout.splitlines() if x.startswith("{")) if l.get("source") == "github:o/r"]
        g1 = grouter()
        g2 = grouter()
        check("github: the first poll only sets the cursor; the next turns a labelled issue into a worker",
              g1 == [] and sorted(l["fate"] for l in g2) == ["addressed to the agent: new worker", "dropped: not addressed to the agent"]
              and any(l.get("thread") == "github:o/r#7" for l in g2))
        gw = next((l.get("session", "") for l in g2 if l.get("session")), "")
        g3 = []
        for _ in range(50):
            g3 += grouter()
            if any(l["fate"].startswith("reply") for l in g3):
                break
            time.sleep(0.2)
        posted = open(os.path.join(home, "gh-posted")).read() if os.path.exists(os.path.join(home, "gh-posted")) else ""
        check("github: the worker's reply is posted on the issue, structured", any(l["fate"] == "reply posted to the issue" and l.get("session") == gw for l in g3)
              and posted.startswith("issue comment 7 -R o/r --body **Coder** finished in") and "gh-ok" in posted)
        g4 = grouter()
        check("github: the Router's own comment is not an event", g4 == [])
        # restart (chapter 20): the Router killed with SIGKILL right after starting a Worker, and right after posting
        # a reply, resumes from its files with no second Worker and no second comment
        khome = tempfile.mkdtemp(prefix="coder-restart-")
        kdir = os.path.join(khome, ".openagents", "coder")
        os.makedirs(os.path.join(kdir, "router"), exist_ok=True)
        with open(os.path.join(kdir, "router", "sources.json"), "w") as f:
            f.write('{"sources":{"local":{"mode":"explicit_requests_only","authors":["me"],"engine":"echo"},'
                    '"github:o/r":{"mode":"explicit_requests_only","authors":["alice"],"label":"coder","engine":"echo","deliver":true}}}')
        kenv = dict(env, HOME=khome, CODER_GH=stub, GH_STUB_DIR=khome)
        def krouter(fault=""):
            e = dict(kenv, CODER_ROUTER_FAULT=fault) if fault else kenv
            r = subprocess.run([coder, "router", "--once", "--json"], cwd=REPO, env=e, capture_output=True, text=True, timeout=300)
            return r.returncode, [json.loads(l) for l in r.stdout.splitlines() if l.startswith("{")]
        def ksessions():
            sd = os.path.join(kdir, "sessions")
            return sorted(n[:-6] for n in os.listdir(sd) if n.endswith(".jsonl")) if os.path.isdir(sd) else []
        subprocess.run([coder, "trigger", "$ echo restart-ok", "--thread", "rs", "--to-agent"], cwd=REPO, env=kenv, capture_output=True, text=True, timeout=60)
        k1 = krouter("started")                     # also the first GitHub poll, which only sets the cursor
        time.sleep(1.0)
        s1 = ksessions()
        pend = os.listdir(os.path.join(kdir, "router", "pending")) if os.path.isdir(os.path.join(kdir, "router", "pending")) else []
        k2 = krouter()
        s2 = ksessions()
        resumed = [l for l in k2[1] if l.get("thread") == "rs" and l["fate"] == "addressed to the agent: new worker"]
        check("restart: killed right after starting a Worker, the Router resumes with the same Worker and starts no other",
              k1[0] in (-9, 137) and len(s1) == 1 and len(pend) == 1 and s2 == s1
              and len(resumed) == 1 and resumed[0].get("session") == s1[0] and "resumed after a restart" in resumed[0].get("note", "")
              and not os.listdir(os.path.join(kdir, "router", "pending")))
        gk = []
        killed = None
        for _ in range(60):                         # the labelled issue starts a Worker; when it ends, the reply is posted and the Router killed
            rc, ls = krouter("posted")
            gk += ls
            if rc in (-9, 137):
                killed = rc
                break
            time.sleep(0.3)
        posts1 = open(os.path.join(khome, "gh-posts")).read().count("post") if os.path.exists(os.path.join(khome, "gh-posts")) else 0
        k3 = krouter()
        k4 = krouter()
        posts2 = open(os.path.join(khome, "gh-posts")).read().count("post") if os.path.exists(os.path.join(khome, "gh-posts")) else 0
        after = [l for l in k3[1] + k4[1] if l.get("source") == "github:o/r"]
        check("restart: killed right after posting a reply, the Router finds its comment by marker and posts no second one",
              killed is not None and posts1 == 1 and posts2 == 1
              and sum(1 for l in after if l["fate"] == "reply posted to the issue") == 1
              and not any("worker" in l["fate"] for l in after))
        # watchdog fault injection (chapter 20): a live Router loop; three stand-in Workers fail at t0 (process killed,
        # one step five times, log stops growing) and the time from each fault to its detection is reported
        whome = tempfile.mkdtemp(prefix="coder-watchdog-")
        wsd = os.path.join(whome, ".openagents", "coder", "sessions"); wrd = os.path.join(whome, ".openagents", "coder", "router")
        os.makedirs(wsd); os.makedirs(wrd)
        with open(os.path.join(wrd, "sources.json"), "w") as f:
            f.write('{"sources":{"local":{"mode":"explicit_requests_only","authors":["me"],"engine":"echo"}}}')
        sl = [subprocess.Popen(["sleep", "600"]) for _ in range(3)]
        wids = ["20261102-000000-101", "20261102-000000-102", "20261102-000000-103"]
        for sid, sp in zip(wids, sl):
            with open(os.path.join(wsd, sid + ".jsonl"), "w") as f:
                f.write('{"kind":"session","id":"%s","engine":"echo","cwd":"/","started_us":1}\n{"kind":"user","text":"fix the parser test"}\n{"kind":"tool","label":"Bash(make test)"}\n' % sid)
            with open(os.path.join(wsd, sid + ".pid"), "w") as f:
                f.write(str(sp.pid))
        with open(os.path.join(wrd, "threads"), "w") as f:
            f.write("".join("w%d\t%s\tlocal\n" % (i, sid) for i, sid in enumerate(wids)))
        wenv = dict(nokey, HOME=whome)
        wr = subprocess.Popen([coder, "router", "--json", "--stuck", "3"], cwd=REPO, env=wenv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        detect = {}
        try:
            for _ in range(8):                        # the Router is up and has made passes over three healthy Workers
                os.utime(os.path.join(wsd, wids[2] + ".jsonl"))
                time.sleep(0.5)
            t0 = time.time()
            sl[0].kill(); sl[0].wait()                                    # lost (reaped, as a detached Worker's process is)
            with open(os.path.join(wsd, wids[1] + ".jsonl"), "a") as f:   # looping: the same step five times
                f.write('{"kind":"tool","label":"Bash(make test)"}\n' * 4)
            os.utime(os.path.join(wsd, wids[2] + ".jsonl"))              # quiet from now on
            want = {wids[0]: "worker lost", wids[1]: "worker redirected: looping", wids[2]: "worker stuck"}
            while time.time() - t0 < 60 and len(detect) < 3:
                if os.path.exists(os.path.join(wrd, "log.jsonl")):
                    for l in open(os.path.join(wrd, "log.jsonl")).read().splitlines():
                        try:
                            j = json.loads(l)
                        except ValueError:
                            continue
                        sid = j.get("session", "")
                        if sid in want and sid not in detect and j.get("fate", "").startswith(want[sid]) and j.get("at_us", 0) / 1e6 >= t0:
                            detect[sid] = j["at_us"] / 1e6 - t0
                time.sleep(0.1)
        finally:
            wr.kill(); wr.wait()
            for sp in sl:
                sp.kill()
        print("     watchdog time to detection: lost %s s, looping %s s, quiet %s s (--stuck 3, passes every 2 s)" % tuple(
            ("%.1f" % detect[w]) if w in detect else "none" for w in wids))
        check("watchdog: a killed, a looping and a quiet Worker are each detected, within a pass after the fault or the --stuck limit",
              len(detect) == 3 and detect[wids[0]] < 4.5 and detect[wids[1]] < 4.5 and 3 <= detect[wids[2]] < 7.5)
        # a CI source: with --ci, a failed workflow run is an event addressed to the agent on the branch's Thread
        pc = subprocess.run([coder, "policy", "add", "github:c/i", "--mode", "explicit_requests_only", "--authors", "*", "--engine", "echo", "--ci"],
                            cwd=REPO, env=genv, capture_output=True, text=True, timeout=120)
        def crouter():
            r = subprocess.run([coder, "router", "--once", "--json"], cwd=REPO, env=genv, capture_output=True, text=True, timeout=300)
            return [l for l in (json.loads(x) for x in r.stdout.splitlines() if x.startswith("{")) if l.get("source") == "github:c/i"]
        c1 = crouter()
        c2 = crouter()
        check("github ci: a failed run starts a worker on the branch's Thread", pc.returncode == 0 and c1 == []
              and any(l["fate"] == "addressed to the agent: new worker" and l.get("thread") == "github:c/i@main" for l in c2))
        c3 = crouter()
        check("github ci: a run seen once is not an event again", not any(l.get("thread") == "github:c/i@main" and "new worker" in l["fate"] for l in c3))
        # coder policy and coder watch: sources are edited by name; a watched Thread's events are addressed to the agent
        def cmd(*a, e=None):
            return subprocess.run([coder, *a], cwd=REPO, env=e or genv, capture_output=True, text=True, timeout=120)
        w1 = cmd("watch", "github:p/q#3")
        pol = json.loads(cmd("policy", "list", "--json").stdout)
        check("watch: a GitHub Thread's repository becomes a source that admits only the gh login",
              w1.returncode == 0 and pol.get("github:p/q") == {"mode": "explicit_requests_only", "authors": ["alice"], "channel": "issue"})
        pa = cmd("policy", "add", "local", "--mode", "explicit_requests_only", "--authors", "me", "--engine", "echo")
        cmd("trigger", "$ echo watched", "--thread", "w")
        def lrouter():
            r = cmd("router", "--once", "--json")
            return [l for l in (json.loads(x) for x in r.stdout.splitlines() if x.startswith("{")) if l.get("source") == "local" and l.get("thread") == "w"]
        before = lrouter()
        cmd("watch", "w")
        cmd("trigger", "$ echo watched", "--thread", "w")
        after = lrouter()
        check("watch: an unaddressed event on a watched Thread starts a worker", pa.returncode == 0
              and [l["fate"] for l in before] == ["dropped: not addressed to the agent"] and [l["fate"] for l in after] == ["addressed to the agent: new worker"])
        cmd("policy", "rm", "local")
        bad = cmd("policy", "add", "x", "--mode", "loud")
        check("policy: rm removes a source; an unknown mode is refused", "local" not in json.loads(cmd("policy", "list", "--json").stdout) and bad.returncode == 2)
        # no --engine and no System One key: the default engine, and the log says why
        bare = dict(env, PATH="/usr/bin:/bin")
        for k in ("BASETEN_API_KEY", "GEMINI_API_KEY", "OPENROUTER_API_KEY", "GOOGLE_APPLICATION_CREDENTIALS", "VERTEX_ACCESS_TOKEN"):
            bare.pop(k, None)
        routed = subprocess.run([coder, "run", "hi", "--json"], cwd=REPO, env=bare, capture_output=True, text=True, timeout=120)
        rl = [json.loads(l) for l in routed.stdout.splitlines() if l.startswith("{")]
        check("run without --engine or a key: the default, and why", len(rl) > 1 and rl[1]["kind"] == "route" and not rl[1]["ok"]
              and rl[1]["ran"] == "microcoder" and "System One key" in rl[1]["text"])
        bn = subprocess.run([coder, "bench", "checks", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=600)
        br = json.loads(bn.stdout) if bn.returncode == 0 else {}
        check("bench: a suite runs and its report is kept", br.get("passed") is True and "passed, 0 failed" in br.get("last", "")
              and len(os.listdir(os.path.join(home, ".openagents", "coder", "bench"))) == 1)
        # coder mcp: the verbs as MCP tools on stdio
        mp = subprocess.Popen([coder, "mcp"], cwd=REPO, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        def rpc(i, m, params):
            mp.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i, "method": m, "params": params}) + "\n"); mp.stdin.flush()
            return json.loads(mp.stdout.readline())
        init = rpc(1, "initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "smoke", "version": "1"}})
        tools = [t["name"] for t in rpc(2, "tools/list", {})["result"]["tools"]]
        started = rpc(3, "tools/call", {"name": "coder_run", "arguments": {"task": "$ echo via-mcp", "engine": "echo"}})["result"]
        mid = started["content"][0]["text"].strip()
        logged = ""
        for _ in range(50):
            logged = rpc(4, "tools/call", {"name": "coder_log", "arguments": {"id": mid}})["result"]["content"][0]["text"]
            if "finished" in logged:
                break
            time.sleep(0.2)
        unknown = rpc(5, "tools/call", {"name": "coder_nope", "arguments": {}})
        res = rpc(6, "resources/list", {})["result"]["resources"]
        mine = [r for r in res if r["name"] == mid]
        read = rpc(7, "resources/read", {"uri": "coder://session/" + mid})
        miss = rpc(8, "resources/read", {"uri": "coder://session/../../etc"})
        mp.stdin.close(); mp.wait(timeout=10)
        check("mcp resources: each session's log at coder://session/ID; another URI is not found",
              init["result"]["capabilities"].get("resources") == {} and len(mine) == 1 and mine[0]["uri"] == "coder://session/" + mid
              and mine[0]["description"].startswith("finished") and "via-mcp" in read["result"]["contents"][0]["text"]
              and miss["error"]["code"] == -32002)
        check("mcp: initialize, the tool list, a run started and its log read through tools",
              init["result"]["serverInfo"]["name"] == "coder" and len(tools) == 11 and "coder_remember" in tools and not started["isError"]
              and "    └ via-mcp" in logged and unknown["error"]["code"] == -32602)
        # coder mcp --http: the same server over Streamable HTTP on 127.0.0.1
        import urllib.request, urllib.error
        hp = subprocess.Popen([coder, "mcp", "--http", "0"], cwd=REPO, env=env, stdout=subprocess.PIPE, text=True)
        try:
            url = hp.stdout.readline().strip().split(" ")[-1]
            def post(body, headers={}, method="POST"):
                rq = urllib.request.Request(url, data=None if method == "GET" else json.dumps(body).encode(), method=method,
                                            headers=dict({"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}, **headers))
                try:
                    with urllib.request.urlopen(rq, timeout=60) as rs:
                        hsid[0] = rs.headers.get("Mcp-Session-Id") or hsid[0]
                        return rs.status, rs.read().decode()
                except urllib.error.HTTPError as e:
                    return e.code, ""
            hsid = [""]
            hnone = post({"jsonrpc": "2.0", "id": 9, "method": "ping"})
            hi = post({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "smoke", "version": "1"}}})
            sh = {"Mcp-Session-Id": hsid[0]}
            hn = post({"jsonrpc": "2.0", "method": "notifications/initialized"}, sh)
            hs = post({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "coder_status", "arguments": {"id": mid}}}, sh)
            hg = post(None, sh, method="GET")
            ho = post({"jsonrpc": "2.0", "id": 3, "method": "ping"}, dict(sh, Origin="https://evil.example"))
            hu = post({"jsonrpc": "2.0", "id": 4, "method": "ping"}, {"Mcp-Session-Id": "0" * 32})
            hd = post(None, sh, method="DELETE")
            ha = post({"jsonrpc": "2.0", "id": 5, "method": "ping"}, sh)
            check("mcp over http: a request answered as JSON, a notification accepted, GET refused, another origin forbidden",
                  hi[0] == 200 and json.loads(hi[1])["result"]["serverInfo"]["name"] == "coder" and hn == (202, "")
                  and hs[0] == 200 and "finished" in json.loads(hs[1])["result"]["content"][0]["text"] and hg[0] == 405 and ho[0] == 403)
            check("mcp over http: initialize starts a session with a random Mcp-Session-Id; no ID is 400, an unknown one 404, DELETE ends it",
                  re.fullmatch(r"[0-9a-f]{32}", hsid[0] or "") is not None and hnone[0] == 400 and hu[0] == 404 and hd[0] == 200 and ha[0] == 404)
        finally:
            hp.kill()
        bad = subprocess.run([coder, "status", "nope"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        check("status of an unknown ID exits 6", bad.returncode == 6)
    finally:
        server.kill()


def main():
    if sys.argv[1:2] == ["--live"]:
        return live(sys.argv[2] if len(sys.argv) > 2 else "claude")
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(REPO)
        env = dict(os.environ, TERM="xterm-256color", OPENAGENTS_SECRETS="/nonexistent")
        for k in ("GEMINI_API_KEY", "OPENROUTER_API_KEY", "GOOGLE_APPLICATION_CREDENTIALS", "VERTEX_ACCESS_TOKEN"):
            env.pop(k, None)
        os.execve(os.path.join(HERE, "coder.sh"), [os.path.join(HERE, "coder.sh"), "--engine", "echo"], env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
    out = bytearray()

    def read_until(needle, seconds):
        end = time.time() + seconds
        while time.time() < end:
            if needle.encode() in shown(out):
                return True
            r, _, _ = select.select([fd], [], [], 0.1)
            if r:
                try:
                    out.extend(os.read(fd, 65536))
                except OSError:
                    break
        return needle.encode() in shown(out)

    def send(b):
        os.write(fd, b)

    checks = []

    def check(name, ok):
        checks.append((name, ok))
        print(("ok   " if ok else "FAIL ") + name)

    # the first build can take a while
    check("banner", read_until("/help for commands.", 600))
    check("composer drawn", read_until("Enter send", 10))
    for ch in "hello coder":
        send(ch.encode()); time.sleep(0.01)
    send(b"\r")
    check("message committed", read_until("› hello coder", 5))
    check("reply streamed", read_until("You said: hello coder", 10))
    check("turn ended", read_until("└ finished", 10))
    send(b"$ echo smoke-ok\r")
    check("command ran", read_until("$ echo smoke-ok · exit 0", 10))
    check("command output", read_until("    smoke-ok", 5))
    send(b"top\x1b[13;2ubottom\r")       # kitty Shift-Enter between two lines
    check("Shift-Enter adds a line", read_until("› top", 5) and read_until("  bottom", 5) and b"topbottom" not in out)
    check("Shift-Enter turn ended", read_until("You said: top", 10))
    end = time.time() + 15
    while out.count("└ finished".encode()) < 3 and time.time() < end:
        read_until("\x00never", 0.2)
    send(b"\x1b[200~first line\rsecond line\x1b[201~")
    send(b"\r")
    check("paste kept both lines", read_until("first line", 5) and read_until("second line", 5))
    check("paste reply", read_until("You said: first line", 10))
    end = time.time() + 15
    while out.count("└ finished".encode()) < 4 and time.time() < end:
        read_until("\x00never", 0.2)
    check("paste turn ended", out.count("└ finished".encode()) >= 4)
    send(b"$ read -r x; echo got-$x\r")   # a command on a terminal reads what the person types (chapter 37)
    time.sleep(1.0)
    send(b"typed-in\r")
    check("a running command takes typed input", read_until("got-typed-in", 10))
    end = time.time() + 10
    while out.count("└ finished".encode()) < 5 and time.time() < end:
        read_until("\x00never", 0.2)
    send(b"/find SMOKE-ok\r")          # search the transcript, any case
    check("/find shows the transcript's matching lines", read_until("⌕     smoke-ok", 5) and read_until("3 lines", 5))
    send(b"/theme light\r")           # a light palette: the brightest text is near black
    mark = len(out)
    check("/theme light", read_until("Theme: light.", 5))
    end = time.time() + 3
    while time.time() < end and b"38;2;20;20;20" not in out[mark:]:
        read_until("\x00never", 0.2)
    check("/theme light draws dark text", b"38;2;20;20;20" in out[mark:])
    send(b"/theme dark\r")
    check("/theme dark", read_until("Theme: dark.", 5))
    # a background job shows in the hint line while it runs
    send(("$ " + os.path.join(HERE, "coder.sh") + " run '$ sleep 8' --engine echo --detach\r").encode())
    check("a running job shows in the hint line", read_until("running · Enter send", 15))
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 60, 0, 0))   # the kernel sends SIGWINCH
    mark = len(out)
    rule = re.compile("(?<!─)(?:─){55,60}(?!─)".encode())
    end = time.time() + 5
    while time.time() < end and not rule.search(shown(out[mark:])):
        read_until("\x00never", 0.2)
    check("resize: the composer redraws at the new width", bool(rule.search(shown(out[mark:]))))
    send(b"\x1b[Z")                    # Shift-Tab, legacy
    check("Shift-Tab to Claude Code", read_until("Agent: Claude Code", 5))
    send(b"\x1b[9;2u")                 # Shift-Tab, kitty
    check("Shift-Tab to Codex", read_until("Agent: Codex", 5))
    check("hint names the agent", read_until("codex · ", 5))
    send(b"\x1b[Z")
    check("Shift-Tab to Microcoder", read_until("Agent: Microcoder", 5) and read_until("microcoder · ", 5))
    send(b"\x1b[A")
    time.sleep(0.3)
    send(b"\x03")            # clears the recalled entry
    time.sleep(0.2)
    send(b"\x03")            # quits
    status = None
    end = time.time() + 5
    while time.time() < end:
        try:
            p, st = os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            break
        if p == pid:
            status = st
            break
        try:
            r, _, _ = select.select([fd], [], [], 0.1)
            if r:
                out.extend(os.read(fd, 65536))
        except OSError:
            pass
    if status is None:
        os.kill(pid, signal.SIGKILL)
    check("Ctrl-C quits with status 0", status is not None and os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0)
    check("terminal restored (bracketed paste off)", b"\x1b[?2004l" in out)
    verbs(check)
    bad = [n for n, ok in checks if not ok]
    if bad:
        sys.stdout.write("--- last output ---\n" + out[-3000:].decode("utf-8", "replace") + "\n")
        print("smoke: FAILED")
        sys.exit(1)
    print("smoke: ok")


if __name__ == "__main__":
    main()
