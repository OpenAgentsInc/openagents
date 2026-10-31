# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent oracle for chapter 175 (a frozen exact core with a learned interface).

Standard library only, sharing no code with psionic/lib/core-interface.coax:
its own machine and program families, its own core (plain integer arithmetic
with the same range), its own sources, controller training, rollout and
first-divergence scoring. It rebuilds psionic/fixtures/core-interface.out line
for line. The core's admission line is the example's own report of chapter
176's checks (pin, cases, recompiled source); the Coax tests check it, and
this oracle prints it as given.
"""
import math
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
CODES = ["push", "get", "set", "add", "sub", "mul", "load", "store", "brif", "out", "ret"]
SOURCES = ["zero", "arg", "top", "second", "local", "depth+1", "depth-1", "outputs+1", "add", "sub", "mul", "branch"]
FAMILIES = ["count", "sum", "stride", "squares", "fact"]
FIELDS = ["ip", "code", "x", "y"]
RANGE = 1 << 26


def program(family, n):
    p = [("push", 2 * n if family == "stride" else n), ("set", 0)]
    loop = {
        "sum": [("push", 0), ("set", 1), ("get", 1), ("get", 0), ("add", 0), ("set", 1), ("get", 0), ("push", 1),
                ("sub", 0), ("set", 0), ("get", 0), ("brif", 4), ("get", 1), ("out", 0), ("ret", 0)],
        "fact": [("push", 1), ("set", 1), ("get", 1), ("get", 0), ("mul", 0), ("set", 1), ("get", 0), ("push", 1),
                 ("sub", 0), ("set", 0), ("get", 0), ("brif", 4), ("get", 1), ("out", 0), ("ret", 0)],
        "squares": [("get", 0), ("get", 0), ("mul", 0), ("out", 0), ("get", 0), ("push", 1), ("sub", 0), ("set", 0),
                    ("get", 0), ("brif", 2), ("ret", 0)],
    }
    step = 2 if family == "stride" else 1
    plain = [("get", 0), ("out", 0), ("get", 0), ("push", step), ("sub", 0), ("set", 0), ("get", 0), ("brif", 2),
             ("ret", 0)]
    return p + loop.get(family, plain)


class State:
    def __init__(self):
        self.ip, self.stack, self.local, self.outs = 0, [], [0] * 8, []

    def features(self, p):
        arg = p[self.ip][1]
        s = self.stack
        return dict(arg=arg, top=s[-1] if s else 0, second=s[-2] if len(s) > 1 else 0,
                    local=self.local[arg] if 0 <= arg < 8 else 0, depth=len(s), outputs=len(self.outs), ip=self.ip)

    def follow(self, p, x, y):
        code, arg = p[self.ip]
        nxt = self.ip + 1
        if code in ("push", "get"):
            self.stack.append(x)
        elif code == "set":
            self.stack.pop()
            self.local[arg] = x
        elif code in ("add", "sub", "mul"):
            self.stack.pop()
            self.stack[-1] = x
        elif code == "brif":
            self.stack.pop()
            if y not in (nxt, arg):
                return False
            nxt = y
        elif code == "out":
            if len(self.outs) >= 64:
                return False
            self.stack.pop()
            self.outs.append(x)
        self.ip = nxt
        return 0 <= nxt < len(p)


def machine(p):
    """The machine's trace as (features, code, x, y) per step."""
    st, rows = State(), []
    while True:
        code, arg = p[st.ip]
        f = st.features(p)
        d, top, second = f["depth"], f["top"], f["second"]
        x = y = 0
        if code == "push":
            x, y = arg, d + 1
        elif code == "get":
            x, y = st.local[arg], arg
        elif code == "set":
            x, y = top, arg
        elif code in ("add", "sub", "mul"):
            x, y = {"add": second + top, "sub": second - top, "mul": second * top}[code], d - 1
        elif code == "brif":
            x, y = top, (arg if top != 0 else st.ip + 1)
        elif code == "out":
            x, y = top, len(st.outs) + 1
        rows.append((f, code, x, y))
        if code == "ret":
            return rows
        st.follow(p, x, y)


def values(f):
    """Each source's value, None where the core refuses."""
    v = [0, f["arg"], f["top"], f["second"], f["local"], f["depth"] + 1, f["depth"] - 1, f["outputs"] + 1]
    core = [None] * 4
    if all(-RANGE <= a <= RANGE for a in (f["second"], f["top"], f["ip"], f["arg"])):
        a, b = f["second"], f["top"]
        core = [a + b, a - b, a * b, f["ip"] + 1 if b == 0 else f["arg"]]
        core = [c if -RANGE <= c <= RANGE else None for c in core]
    return v + core


def softmax(logits):
    top = max(logits)
    p = [math.exp(v - top) for v in logits]
    total = 0.0
    for v in p:
        total += v
    return [v / total for v in p]


def train(examples, epochs, rate):
    masks = []
    for f, code, x, y in examples:
        v = values(f)
        masks.append([{s for s in range(12) if v[s] is not None and v[s] == want} for want in (x, y)])
    rows = [(e, m) for e, m in zip(examples, masks) if m[0] and m[1]]
    logits = {}
    for c in CODES:
        mine = [m for e, m in rows if e[1] == c]
        if not mine:
            continue
        for t in range(2):
            lg = [0.0] * 12
            for _ in range(epochs):
                p = softmax(lg)
                grad = [0.0] * 12
                for m in mine:
                    mass = 0.0
                    for s in range(12):
                        if s in m[t]:
                            mass += p[s]
                    for s in range(12):
                        grad[s] += p[s]
                        if s in m[t]:
                            grad[s] -= p[s] / mass
                lg = [lg[s] - rate * grad[s] / len(mine) for s in range(12)]
            logits[c, t] = lg
    total = 0.0
    for e, m in rows:
        for t in range(2):
            p = softmax(logits[e[1], t])
            mass = 0.0
            for s in range(12):
                if s in m[t]:
                    mass += p[s]
            total += -math.log(mass)
    return logits, total / (2 * len(rows))


def route(logits, code, t):
    lg = logits.get((code, t), [0.0] * 12)
    best = 0
    for s in range(1, 12):
        if lg[s] > lg[best]:
            best = s
    return best, [s for s in range(12) if s != best and lg[s] == lg[best]]


def score(logits, p):
    """('exact', ...), ('wrong', token, step, wrote, want) or ('refused', step); with the machine's length."""
    truth = machine(p)
    want = [t for f, c, x, y in truth for t in (f["ip"], CODES.index(c), x, y)]
    st, got = State(), []
    while len(got) < len(want):
        code = p[st.ip][0]
        v = values(st.features(p))
        rx, ry = route(logits, code, 0)[0], route(logits, code, 1)[0]
        if v[rx] is None or v[ry] is None:
            break
        got += [st.ip, CODES.index(code), v[rx], v[ry]]
        if code == "ret" or not st.follow(p, v[rx], v[ry]):
            break
    for i, (a, b) in enumerate(zip(got, want)):
        if a != b:
            return ("wrong", i, i // 4, a, b), truth
    if len(got) == len(want):
        return ("exact",), truth
    return ("refused", len(got) // 4), truth


def r4(v):
    return math.floor(v * 10000 + 0.5) / 10000


def controller(name, families, out):
    examples = []
    for fam in families:
        for n in (1, 2, 3, 4):
            examples += machine(program(fam, n))
    names = " " + families[0] + "".join((" and " if i == len(families) - 1 else ", ") + f
                                        for i, f in enumerate(families) if i)
    out.append(f"controller {name}: trained on{names} at n = 1..4: {4 * len(families)} traces admitted by chapter 186, "
               f"{len(examples)} rows")
    logits, loss = train(examples, 200, 1.0)
    out.append(f"  loss {r4(loss)}")
    out.append("  routes, with sources training never told apart after a slash:")
    line = "   "
    for c in CODES:
        if (c, 0) not in logits:
            continue
        line += f" {c}"
        for t in range(2):
            best, ties = route(logits, c, t)
            line += (" " if t == 0 else ", ") + "/".join(SOURCES[s] for s in [best] + ties)
        line += ";"
    out.append(line)
    out.append("    never seen:" + "".join(f" {c}" for c in CODES if (c, 0) not in logits))
    tally = {"exact": 0, "refused": 0, "wrong": 0}
    for fam in FAMILIES:
        ns = (4, 8, 11, 12) if fam == "fact" else (4, 8, 16, 32)
        parts = []
        for n in ns:
            r, truth = score(logits, program(fam, n))
            tally[r[0]] += 1
            if r[0] == "exact":
                parts.append(f"n={n} exact")
            elif r[0] == "refused":
                parts.append(f"n={n} refused at step {r[1]} of {len(truth)}")
            else:
                parts.append(f"n={n} token {r[1]} of {4 * len(truth)}")
        out.append(f"  {fam}: " + ", ".join(parts))
        if r[0] == "wrong":
            out.append(f"    first miss at n={ns[3]}: step {r[2]} ({truth[r[2]][1]}) wrote {r[3]} for {FIELDS[r[1] % 4]}, "
                       f"where the machine has {r[4]}")
        elif r[0] == "refused":
            out.append(f"    at n={ns[3]}, step {r[1]} ({truth[r[1]][1]}) needs a core output past 2^26")
    return tally


def example():
    out = ["core: chapter 168's block for inputs second, top, ip and arg, with outputs second + top,",
           "  second - top, second * top, and ip + 1 when top is 0, else arg; exact within 2^26",
           "  chapter 176's admission: exact (pin, 4 cases, source recompiled to the pin)",
           "interface: each token is copied from one of 12 sources:",
           "  " + ", ".join(SOURCES),
           "controller: per instruction and token, a softmax over the sources that sees only the instruction,",
           "trained from zero by 200 full-batch steps on -log P(a source that gives the machine's token)"]
    a = controller("A", ["count", "sum"], out)
    b = controller("B", ["count", "sum", "stride", "fact"], out)
    out.append(f"of 20 runs each: A {a['exact']} exact, {a['refused']} refused, {a['wrong']} wrong; "
               f"B {b['exact']} exact, {b['refused']} refused, {b['wrong']} wrong")
    return out


if __name__ == "__main__":
    out = example()
    fixture = (ROOT / "psionic/fixtures/core-interface.out").read_text().splitlines()
    for n, (a, b) in enumerate(zip(out, fixture)):
        assert a == b, f"line {n + 1}:\n  oracle  {a}\n  fixture {b}"
    assert len(out) == len(fixture), (len(out), len(fixture))
    print(f"core-interface oracle: both controllers retrained independently; the example's {len(out)} lines are "
          "rebuilt line for line")
