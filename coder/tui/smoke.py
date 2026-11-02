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
import os, sys
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
            print('{"k":"comment:999","c":"999","n":7,"a":"alice","t":"the posted reply"}')
    else:
        print('{"k":"issue:7","n":7,"a":"alice","t":"$ echo gh-ok"}')
elif a[:2] == ["issue", "comment"]:
    open(os.path.join(d, "gh-posted"), "w").write(" ".join(a))
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
        check("a disabled plugin keeps no command", pl == [{"name": "demo", "state": "disabled", "signed": False, "commands": []}])
        # a plugin's hooks run on their events, in the session's folder, with the session's context
        with open(os.path.join(plugdir, "hooks.json"), "w") as f:
            f.write('{"name":"hooks","hooks":[{"on":"session_start","run":"echo start-$CODER_EVENT"},{"on":"turn_end","run":"echo end-$CODER_ENDING-$CODER_ENGINE"}]}')
        hk = subprocess.run([coder, "run", "hi", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        hl = [json.loads(l) for l in hk.stdout.splitlines() if '"kind":"hook"' in l]
        check("a plugin's hooks run on session_start and turn_end", [(h["on"], h["text"]) for h in hl] == [("session_start", "start-session_start"), ("turn_end", "end-finished-echo")])
        os.remove(os.path.join(plugdir, "hooks.json"))
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
        mp.stdin.close(); mp.wait(timeout=10)
        check("mcp: initialize, the tool list, a run started and its log read through tools",
              init["result"]["serverInfo"]["name"] == "coder" and len(tools) == 8 and not started["isError"]
              and "    └ via-mcp" in logged and unknown["error"]["code"] == -32602)
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
