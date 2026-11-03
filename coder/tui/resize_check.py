#!/usr/bin/env python3
"""Resizes Coder's terminal UI while a turn is working and checks the screen.

Runs coder/tui/coder.sh on the offline echo engine in a real pseudo-terminal
(120x30), runs "$ sleep 30" so the Working row shows, then narrows the
terminal in steps (TIOCSWINSZ, so Coder gets SIGWINCH) and widens it again.
Coder's output feeds a small terminal model that reflows on a width change
the way Ghostty (the Rex app's terminal), iTerm2, kitty and Terminal.app do:
a line wider than the new width wraps onto more rows, the bottom of the
screen stays put and rows pushed off the top go into the scrollback.

After each resize the visible screen must hold one Working row, two rules,
one composer and one footer, with the footer on the last row: no stale
status rows, no rule fragments, no wrapped leftovers of the old rows.

    python3 coder/tui/resize_check.py          # prints the screens, exits 1 on a failure
    python3 coder/tui/resize_check.py --quiet  # only the verdicts

Test-only harness; the program under test is Coax (coax/lib/tui/tui.coax,
coder/tui/coder.coax).
"""
import fcntl, os, pty, re, select, signal, struct, sys, tempfile, termios, time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
CSI = re.compile(r"\x1b\[([0-9;?<>=]*)([ -/]*)([@-~])")


class Term:
    """A reflowing terminal: rows of cells, a soft-wrap flag per row, the
    screen the last `rows` rows of the buffer."""

    def __init__(self, cols, rows):
        self.cols, self.rows = cols, rows
        self.buf = [[] for _ in range(rows)]      # each row: list of chars ("" is an empty cell)
        self.wrap = [False] * rows                # the row continues on the next (a soft wrap)
        self.y, self.x, self.pending = 0, 0, False
        self.rest = ""

    def top(self):
        return len(self.buf) - self.rows

    def row(self, y):
        return self.buf[self.top() + y]

    def line_feed(self):
        if self.y == self.rows - 1:
            self.buf.append([]); self.wrap.append(False)
        else:
            self.y += 1

    def put(self, ch):
        if self.pending:
            self.wrap[self.top() + self.y] = True
            self.line_feed(); self.x = 0; self.pending = False
        r = self.row(self.y)
        while len(r) <= self.x:
            r.append("")
        r[self.x] = ch
        if self.x == self.cols - 1:
            self.pending = True
        else:
            self.x += 1

    def erase_right(self, y, x):
        r = self.row(y)
        del r[x:]
        self.wrap[self.top() + y] = False

    def csi(self, params, final):
        priv = params.startswith(("?", "<", ">", "="))
        nums = [int(p) if p.isdigit() else 0 for p in params.lstrip("?<>=").split(";")] if params else []
        n = nums[0] if nums and nums[0] > 0 else 1
        if priv:
            return
        if final == "A":
            self.y = max(0, self.y - n); self.pending = False
        elif final == "B":
            self.y = min(self.rows - 1, self.y + n); self.pending = False
        elif final == "C":
            self.x = min(self.cols - 1, self.x + n); self.pending = False
        elif final == "D":
            self.x = max(0, self.x - n); self.pending = False
        elif final in "Hf":
            r = nums[0] if nums and nums[0] > 0 else 1
            c = nums[1] if len(nums) > 1 and nums[1] > 0 else 1
            self.y, self.x, self.pending = min(r, self.rows) - 1, min(c, self.cols) - 1, False
        elif final == "K":
            if not nums or nums[0] == 0:
                self.erase_right(self.y, self.x)
            elif nums[0] == 2:
                self.erase_right(self.y, 0)
        elif final == "J":
            mode = nums[0] if nums else 0
            if mode == 0:
                self.erase_right(self.y, self.x)
                for y in range(self.y + 1, self.rows):
                    self.erase_right(y, 0)
            elif mode in (2, 3):
                for y in range(self.rows):
                    self.erase_right(y, 0)

    def feed(self, data):
        s = self.rest + data; self.rest = ""
        i = 0
        while i < len(s):
            c = s[i]
            if c == "\x1b":
                m = CSI.match(s, i)
                if m:
                    self.csi(m.group(1), m.group(3)); i = m.end(); continue
                if i + 1 >= len(s) or (s[i + 1] == "[" and re.fullmatch(r"\x1b\[[0-9;?<>=]*[ -/]*", s[i:])):
                    self.rest = s[i:]; return            # the sequence goes on in the next read
                if s[i + 1] == "]":                 # OSC ... BEL or ST
                    e = s.find("\x07", i); st = s.find("\x1b\\", i)
                    ends = [p for p in (e, st) if p >= 0]
                    if not ends:
                        self.rest = s[i:]; return
                    p = min(ends); i = p + (1 if p == e else 2); continue
                i += 2; continue
            if c == "\r":
                self.x = 0; self.pending = False
            elif c == "\n":
                self.line_feed(); self.pending = False
            elif c == "\b":
                self.x = max(0, self.x - 1); self.pending = False
            elif c >= " ":
                self.put(c)
            i += 1

    def resize(self, cols, rows):
        """Reflow: rejoin soft-wrapped rows into lines, wrap each at the new
        width (trailing empty cells dropped, the cursor's cell kept), keep the
        cursor on its cell and the bottom of the buffer at the screen's bottom."""
        cy = self.top() + self.y
        lines, cur = [], None
        k = 0
        while k < len(self.buf):
            start = k; cells = []
            while True:
                if k == cy:
                    cur = (len(lines), len(cells) + self.x)
                cells.extend(self.buf[k] + [""] * (self.cols - len(self.buf[k])) if self.wrap[k] else self.buf[k])
                if not self.wrap[k] or k == len(self.buf) - 1:
                    break
                k += 1
            k += 1
            while cells and cells[-1] == "":
                cells.pop()
            lines.append(cells)
        buf, wrap, ny, nx = [], [], 0, 0
        for li, cells in enumerate(lines):
            need = len(cells)
            if cur and cur[0] == li:
                need = max(need, cur[1] + 1)
            nrows = max(1, (need + cols - 1) // cols)
            for j in range(nrows):
                buf.append(cells[j * cols:(j + 1) * cols]); wrap.append(j < nrows - 1)
            if cur and cur[0] == li:
                ny = len(buf) - nrows + cur[1] // cols; nx = cur[1] % cols
        while len(buf) < rows:
            buf.append([]); wrap.append(False)
        self.buf, self.wrap, self.cols, self.rows = buf, wrap, cols, rows
        self.y = max(0, min(rows - 1, ny - self.top())); self.x = nx; self.pending = False

    def count(self, text):
        """Rows anywhere in the buffer that hold text."""
        return sum(1 for r in self.buf if text in "".join(ch or " " for ch in r))

    def screen(self):
        return ["".join(ch or " " for ch in self.row(y)).rstrip() for y in range(self.rows)]


def spawn(cols, rows):
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(REPO)
        home = tempfile.mkdtemp(prefix="coder-resize-")
        env = dict(os.environ, HOME=home, TERM="xterm-256color", BROWSER="/usr/bin/true", NO_OPEN_BROWSER="1",
                   CODER_USAGE="0", CODER_USAGE_KEYCHAIN="0")
        os.execve(os.path.join(HERE, "coder.sh"), [os.path.join(HERE, "coder.sh"), "--engine", "echo"], env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    return pid, fd


def session(prefix, fill, quiet, failures):
    """One Coder session: fill the screen with transcript first when fill
    is set, then work and drag the terminal's edges."""
    cols, rows = 120, 30
    term = Term(cols, rows)
    pid, fd = spawn(cols, rows)
    raw = open(os.environ["RESIZE_RAW"] + "." + prefix.strip(": "), "wb") if os.environ.get("RESIZE_RAW") else None   # Coder's bytes, for debugging
    dec = __import__("codecs").getincrementaldecoder("utf-8")("replace")

    def pump(seconds, until=None):
        end = time.time() + seconds
        while time.time() < end:
            r, _, _ = select.select([fd], [], [], 0.05)
            if r:
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    return False
                if raw:
                    raw.write(data)
                term.feed(dec.decode(data))
            if until and any(until in l for l in term.screen()):
                return True
        return until is None

    def resize(c, r):
        # as a terminal does: take in what has arrived, reflow, then tell the program
        while select.select([fd], [], [], 0)[0]:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if raw:
                raw.write(data)
            term.feed(dec.decode(data))
        term.resize(c, r)
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", r, c, 0, 0))

    def check(name, good, screen=None):
        print(("ok   " if good else "FAIL ") + prefix + name)
        if not good:
            failures.append(prefix + name)
            if screen is not None and quiet:
                print("\n".join("  |" + l for l in screen))

    def settle():
        # a frame comes in pieces (a pseudo-terminal passes about 1 KB at a time); read until 40 ms of quiet
        while True:
            r, _, _ = select.select([fd], [], [], 0.04)
            if not r:
                return
            pump(0.01)

    def verdict(label):
        settle()
        s = term.screen()
        if not quiet:
            print("---- " + prefix + label + " (%dx%d)" % (term.cols, term.rows))
            print("\n".join("|" + l for l in s))
        working = [l for l in s if "Esc to stop" in l]
        rules = [l for l in s if l and set(l) == {"─"}]
        footers = [l for l in s if "Enter send" in l]
        stale = [l for l in s if l.strip() and "to stop" in l and "Esc" not in l]
        check(label + ": one Working row", len(working) == 1, s)
        check(label + ": two whole rules, no fragments", len(rules) == 2 and all(len(l) == term.cols - 1 for l in rules), s)
        check(label + ": one footer, on the last row", len(footers) == 1 and "Enter send" in s[-1], s)
        check(label + ": no wrapped leftovers", not stale, s)
        if fill:
            # the transcript stays on the screen above the region: its last line is still in view, once
            check(label + ": the transcript above is kept", sum(1 for l in s if l.startswith("› $ sleep 30")) == 1, s)

    def send(text, until):
        os.write(fd, text.encode()); pump(0.3)
        os.write(fd, b"\r")
        return pump(30, until) if until else pump(0.2)

    try:
        check("banner", pump(120, "/help for commands."))
        if fill:
            for i in range(1, 9):
                n = term.count("└ finished")
                send("message %d: a long line of text, so that the echo reply wraps across the width of this terminal window" % i, None)
                end = time.time() + 30
                while time.time() < end and term.count("└ finished") <= n:
                    pump(0.1)
                pump(0.3)
        check("Working shows", send("$ sleep 30", "Esc to stop"))
        pump(0.5)
        verdict("120 columns")
        for c in (100, 80, 64, 50):        # a drag: several widths in a row
            resize(c, rows)
            pump(0.15)
        pump(0.8)
        verdict("narrowed to 50")
        resize(50, 24); pump(0.8)
        verdict("shorter, 50x24")
        for c in (70, 96, 120):
            resize(c, 24); pump(0.15)
        pump(0.8)
        verdict("widened to 120")
    finally:
        for key in (b"\x1b", b"\x03", b"\x03", b"\x03"):    # stop the command, then quit
            try:
                os.write(fd, key)
            except OSError:
                break
            pump(0.3)
        end = time.time() + 10
        done = False
        while time.time() < end and not done:
            p, _ = os.waitpid(pid, os.WNOHANG)
            done = p == pid
            if not done:
                pump(0.1)
        if not done:
            os.kill(pid, signal.SIGKILL); os.waitpid(pid, 0)
        os.close(fd)


def main():
    quiet = "--quiet" in sys.argv
    failures = []
    session("early: ", False, quiet, failures)   # blank rows keep the composer on the last row
    session("full: ", True, quiet, failures)     # the transcript fills the screen, as in a long session
    print("resize check: " + ("ok" if not failures else "FAILED (%d)" % len(failures)))
    sys.exit(0 if not failures else 1)


if __name__ == "__main__":
    main()
