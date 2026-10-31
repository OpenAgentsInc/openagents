#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 232's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/replay.coax or chapter 224's interpreter:
its own machine, its own digests with hashlib, its own fold and replay. It
rebuilds every scenario in psionic/examples/exact-replay.coax and prints the
same lines, so the two outputs can be compared byte for byte.
"""
import hashlib
import struct
import sys

CODES = ["push", "get", "set", "add", "sub", "mul", "load", "store", "brif", "out", "ret"]
POPS = {"set": 1, "add": 2, "sub": 2, "mul": 2, "load": 1, "store": 2, "brif": 1, "out": 1}
I32 = (-2 ** 31, 2 ** 31 - 1)

SQUARES = [("push", 4), ("set", 0), ("get", 0), ("get", 0), ("get", 0), ("mul", 0), ("store", 0),
           ("get", 0), ("push", 1), ("sub", 0), ("set", 0), ("get", 0), ("brif", 2),
           ("push", 4), ("load", 0), ("push", 1), ("load", 0), ("add", 0), ("out", 0), ("ret", 0)]


def le(values):
    return b"".join(struct.pack("<I", v & 0xFFFFFFFF) for v in values)


def program_digest(program):
    return hashlib.sha256(b"".join(bytes([CODES.index(c)]) + le([a]) for c, a in program)).digest()


def ints_digest(values):
    return hashlib.sha256(le(values)).digest()


class Machine:
    def __init__(self):
        self.stack, self.locals, self.cells, self.outputs = [], [0] * 8, [0] * 64, []


def step(program, ip, m):
    """One instruction on m; returns (row, next ip) or None when refused."""
    code, arg = program[ip]
    if len(m.stack) < POPS.get(code, 0) or (code in ("push", "get") and len(m.stack) >= 32):
        return None
    nxt, x, y = ip + 1, 0, 0
    if code == "push":
        x = arg; m.stack.append(x); y = len(m.stack)
    elif code == "get":
        x = m.locals[arg]; m.stack.append(x); y = arg
    elif code == "set":
        x = m.stack.pop(); m.locals[arg] = x; y = arg
    elif code in ("add", "sub", "mul"):
        b, a = m.stack.pop(), m.stack.pop()
        x = {"add": a + b, "sub": a - b, "mul": a * b}[code]
        if not I32[0] <= x <= I32[1]:
            return None
        m.stack.append(x); y = len(m.stack)
    elif code == "load":
        y = m.stack.pop()
        if not 0 <= y < 64:
            return None
        x = m.cells[y]; m.stack.append(x)
    elif code == "store":
        x, y = m.stack.pop(), m.stack.pop()
        if not 0 <= y < 64:
            return None
        m.cells[y] = x
    elif code == "brif":
        x = m.stack.pop(); nxt = arg if x != 0 else ip + 1; y = nxt
    elif code == "out":
        if len(m.outputs) >= 64:
            return None
        x = m.stack.pop(); m.outputs.append(x); y = len(m.outputs)
    else:
        nxt = -1
    return [ip, CODES.index(code), x, y], nxt


def run(program, budget):
    m, ip, rows = Machine(), 0, []
    while len(rows) < budget:
        row, nxt = step(program, ip, m)
        rows.append(row)
        if nxt < 0:
            return rows, m.outputs
        ip = nxt
    raise ValueError("budget")


def fold(program, rows, upto):
    """The machine before row `upto`, taking each row's x on trust; or the failing step."""
    m, ip = Machine(), 0
    for i in range(upto):
        rip, code, x, y = rows[i]
        if ip >= len(program) or rip != ip or code != CODES.index(program[ip][0]):
            return i, None, None
        name, arg = program[ip]
        if len(m.stack) < POPS.get(name, 0) or name == "ret":
            return i, None, None
        nxt = ip + 1
        if name in ("push", "get"):
            if len(m.stack) >= 32:
                return i, None, None
            m.stack.append(x)
        elif name == "set":
            m.stack.pop(); m.locals[arg] = x
        elif name in ("add", "sub", "mul"):
            m.stack.pop(); m.stack[-1] = x
        elif name == "load":
            m.stack[-1] = x
        elif name == "store":
            m.stack.pop(); a = m.stack.pop()
            if not 0 <= a < 64:
                return i, None, None
            m.cells[a] = x
        elif name == "brif":
            if y not in (ip + 1, arg):
                return i, None, None
            m.stack.pop(); nxt = y
        elif name == "out":
            if len(m.outputs) >= 64:
                return i, None, None
            m.stack.pop(); m.outputs.append(x)
        ip = nxt
    return upto, m, ip


def check(program, claim, rows, a, b, budget):
    """Returns (verdict, reason, step, field, matched)."""
    steps = claim["steps"]
    if program_digest(program) != claim["program"]:
        return "failed", "program", -1, None, 0
    if b - a > budget:
        return "unverifiable", "over_budget", -1, None, 0
    whole = a == 0 and b == steps
    if rows is None and not whole:
        return "unverifiable", "no_rows", -1, None, 0
    if rows is not None and ints_digest([t for r in rows for t in r]) != claim["trace"]:
        return "failed", "rows", -1, None, 0
    at, m, ip = fold(program, rows or [], a)
    if m is None:
        return "failed", "structure", at, None, 0
    produced, outs, matched = [], [], 0
    for s in range(a, b):
        if ip >= len(program):
            return "failed", "refused", s, None, matched
        before = len(m.outputs)
        got = step(program, ip, m)
        if got is None:
            return "failed", "refused", s, None, matched
        row, nxt = got
        if rows is not None and row != rows[s]:
            field = ["ip", "code", "x", "y"][[i for i in range(4) if row[i] != rows[s][i]][0]]
            return "failed", "row", s, field, matched
        matched += 1
        produced.extend(row)
        if len(m.outputs) > before:
            outs.append(row[2])
        if nxt < 0 and s < steps - 1:
            return "failed", "halted", s, None, matched
        if nxt >= 0 and s == steps - 1:
            return "failed", "running", s, None, matched
        ip = nxt
    if not whole:
        return "unverifiable", "window", -1, None, matched
    if ints_digest(produced) != claim["trace"]:
        return "failed", "trace", -1, None, matched
    if ints_digest(outs) != claim["outputs"]:
        return "failed", "outputs", -1, None, matched
    return "exact", "none", -1, None, matched


def flip(d):
    return bytes([d[0] ^ 1]) + d[1:]


def show(what, r):
    verdict, reason, s, field, matched = r
    line = "%s: %s (%s)" % (what, verdict, reason)
    if s >= 0:
        line += " at step %d" % s
    if field:
        line += ", field %s" % field
    print(line + ", %d steps replayed" % matched)


def main():
    p = SQUARES
    rows, outs = run(p, 64)
    claim = dict(program=program_digest(p), trace=ints_digest([t for r in rows for t in r]),
                 outputs=ints_digest(outs), steps=len(rows))
    print("claim: %d steps, output %d" % (len(rows), outs[0]))
    show("whole run, with rows", check(p, claim, rows, 0, 53, 64))
    show("whole run, digests only", check(p, claim, None, 0, 53, 64))
    show("window 40..<53", check(p, claim, rows, 40, 53, 64))
    show("whole run, budget 20", check(p, claim, rows, 0, 53, 20))
    for key, what in (("program", "program digest changed"), ("trace", "trace digest changed"),
                      ("outputs", "output digest changed")):
        show(what, check(p, dict(claim, **{key: flip(claim[key])}), rows, 0, 53, 64))
    forged = [list(r) for r in rows]
    forged[6][2] = 99
    t = dict(claim, trace=ints_digest([x for r in forged for x in r]))
    show("store forged, whole run", check(p, t, forged, 0, 53, 64))
    show("store forged, window 40..<53", check(p, t, forged, 40, 53, 64))
    show("store forged, window 48..<53", check(p, t, forged, 48, 53, 64))
    short = rows[:52]
    t = dict(claim, steps=52, trace=ints_digest([x for r in short for x in r]))
    show("claim of 52 steps", check(p, t, short, 0, 52, 64))


if __name__ == "__main__":
    sys.exit(main())
