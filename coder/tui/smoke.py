#!/usr/bin/env python3
"""Drives coder/tui/coder.sh in a real pseudo-terminal (100x30) on the
offline echo engine: types a message and checks the streamed reply, runs
"$ echo ..." and checks the command row, pastes two lines with bracketed
paste, walks history with Up, switches agent with Shift-Tab (legacy and
kitty), then quits with Ctrl-C and checks the exit. Then the verbs without a
terminal, in a fresh HOME: coder run --json on the psionic engine against a
stand-in local model server (local_model.py), coder ls --json, coder log and
coder status on an unknown ID, coder fork, coder run --detach then coder
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
        check("run --json: session, user, tool, result, reply, end", kinds == ["session", "user", "tool", "result", "reply", "end"])
        check("run --json: the local model answered", any(l.get("text") == "local-ok" for l in lines) and lines[-1].get("ending") == "finished")
        ls = subprocess.run([coder, "ls", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        rows = json.loads(ls.stdout) if ls.returncode == 0 else []
        check("ls --json lists the session", len(rows) == 1 and rows[0]["engine"] == "psionic" and rows[0]["state"] == "finished" and rows[0]["steps"] == 1)
        sid = rows[0]["id"] if rows else "none"
        log = subprocess.run([coder, "log", sid[:15]], cwd=REPO, env=env, capture_output=True, text=True, timeout=60)
        check("log by ID prefix", log.returncode == 0 and "● local-ok" in log.stdout and "Bash(echo local-step)" in log.stdout)
        # fork: the new session carries the parent's turns, and its header names the parent
        first = subprocess.run([coder, "run", "remember heliotrope", "--engine", "echo", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        pid_ = json.loads(first.stdout.splitlines()[0])["id"]
        fork = subprocess.run([coder, "fork", pid_, "what word?", "--json"], cwd=REPO, env=env, capture_output=True, text=True, timeout=120)
        fl = [json.loads(l) for l in fork.stdout.splitlines() if l.startswith("{")]
        check("fork carries the parent's turns", fork.returncode == 0 and fl[0].get("parent") == pid_ and fl[0].get("at") == 1
              and any("User: remember heliotrope" in l.get("text", "") and "Now: what word?" in l.get("text", "") for l in fl if l["kind"] == "reply"))
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
        # no --engine and no System One key: the default engine, and the log says why
        bare = dict(env, PATH="/usr/bin:/bin")
        for k in ("BASETEN_API_KEY", "GEMINI_API_KEY", "OPENROUTER_API_KEY", "GOOGLE_APPLICATION_CREDENTIALS", "VERTEX_ACCESS_TOKEN"):
            bare.pop(k, None)
        routed = subprocess.run([coder, "run", "hi", "--json"], cwd=REPO, env=bare, capture_output=True, text=True, timeout=120)
        rl = [json.loads(l) for l in routed.stdout.splitlines() if l.startswith("{")]
        check("run without --engine or a key: the default, and why", len(rl) > 1 and rl[1]["kind"] == "route" and not rl[1]["ok"]
              and rl[1]["ran"] == "microcoder" and "System One key" in rl[1]["text"])
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
    send(b"\x1b[Z")                    # Shift-Tab, legacy
    check("Shift-Tab to Claude Code", read_until("Agent: Claude Code", 5))
    send(b"\x1b[9;2u")                 # Shift-Tab, kitty
    check("Shift-Tab to Codex", read_until("Agent: Codex", 5))
    check("hint names the agent", read_until("codex · Enter send", 5))
    send(b"\x1b[Z")
    check("Shift-Tab to Microcoder", read_until("Agent: Microcoder", 5) and read_until("microcoder · Enter send", 5))
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
