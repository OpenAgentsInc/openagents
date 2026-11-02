#!/usr/bin/env python3
"""Drives coder/tui/coder.sh in a real pseudo-terminal (100x30) on the
offline echo engine: types a message and checks the streamed reply, runs
"$ echo ..." and checks the command row, pastes two lines with bracketed
paste, walks history with Up, then quits with Ctrl-C and checks the exit.

    python3 coder/tui/smoke.py          # offline, echo engine
    python3 coder/tui/smoke.py --live   # two turns on the claude engine (your Claude Code login)

Test-only harness; the program under test is Coax.
"""
import os, pty, re, select, signal, struct, sys, time, fcntl, termios

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


def live():
    """Two turns in one Claude Code session: the second needs the first."""
    pid, fd = spawn("claude")
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
    steps.append(("first turn", read_until("● noted", 120) and read_until("⎿ finished", 30)))
    os.write(fd, b"What word did I ask you to remember? Reply with only that word.\r")
    steps.append(("second turn remembers", read_until("● heliotrope", 120)))
    end = time.time() + 30
    while out.count("⎿ finished".encode()) < 2 and time.time() < end:
        read_until("\x00never", 0.2)
    steps.append(("second turn ended", out.count("⎿ finished".encode()) >= 2))
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


def main():
    if sys.argv[1:] == ["--live"]:
        return live()
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
    check("turn ended", read_until("⎿ finished", 10))
    send(b"$ echo smoke-ok\r")
    check("command ran", read_until("$ echo smoke-ok · exit 0", 10))
    check("command output", read_until("    smoke-ok", 5))
    send(b"top\x1b[13;2ubottom\r")       # kitty Shift-Enter between two lines
    check("Shift-Enter adds a line", read_until("› top", 5) and read_until("  bottom", 5) and b"topbottom" not in out)
    check("Shift-Enter turn ended", read_until("You said: top", 10))
    end = time.time() + 15
    while out.count("⎿ finished".encode()) < 3 and time.time() < end:
        read_until("\x00never", 0.2)
    send(b"\x1b[200~first line\rsecond line\x1b[201~")
    send(b"\r")
    check("paste kept both lines", read_until("first line", 5) and read_until("second line", 5))
    check("paste reply", read_until("You said: first line", 10))
    end = time.time() + 15
    while out.count("⎿ finished".encode()) < 4 and time.time() < end:
        read_until("\x00never", 0.2)
    check("paste turn ended", out.count("⎿ finished".encode()) >= 4)
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
    bad = [n for n, ok in checks if not ok]
    if bad:
        sys.stdout.write("--- last output ---\n" + out[-3000:].decode("utf-8", "replace") + "\n")
        print("smoke: FAILED")
        sys.exit(1)
    print("smoke: ok")


if __name__ == "__main__":
    main()
