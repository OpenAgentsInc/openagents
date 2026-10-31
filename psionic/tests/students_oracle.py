# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent oracle for chapter 233 (can gradient descent learn exact execution?).

Standard library only, sharing no code with psionic/lib/students.coax: its own
stack machine and program families, its own fold of a student's rows, its own
digests with hashlib, and its own students. A student that the example trains
to convergence is computed here in exact rational arithmetic as the limit
gradient descent from zero reaches, the minimum-norm least-squares fit, so the
oracle checks the example's numbers without repeating its arithmetic. The
student stopped early (gated, 10000 epochs) is retrained here in binary64 from
the same gradient written as a Gram matrix. It rebuilds
psionic/fixtures/students.out line for line.
"""
import hashlib
import math
import pathlib
import struct
from fractions import Fraction

ROOT = pathlib.Path(__file__).resolve().parents[2]
CODES = ["push", "get", "set", "add", "sub", "mul", "load", "store", "brif", "out", "ret"]
FAMILIES = ["count", "sum", "stride", "squares"]
FIELDS = ["ip", "code", "x", "y"]
TESTS = (4, 8, 16, 32)
SPAN = 64
WIDTH = {"linear": 8, "gated": 16, "table": 1 + 7 * (2 * SPAN + 1)}


def program(family, n):
    p = [("push", 2 * n if family == "stride" else n), ("set", 0)]
    if family == "sum":
        p += [("push", 0), ("set", 1), ("get", 1), ("get", 0), ("add", 0), ("set", 1), ("get", 0), ("push", 1),
              ("sub", 0), ("set", 0), ("get", 0), ("brif", 4), ("get", 1), ("out", 0), ("ret", 0)]
    elif family == "squares":
        p += [("get", 0), ("get", 0), ("mul", 0), ("out", 0), ("get", 0), ("push", 1), ("sub", 0), ("set", 0),
              ("get", 0), ("brif", 2), ("ret", 0)]
    else:
        p += [("get", 0), ("out", 0), ("get", 0), ("push", 2 if family == "stride" else 1), ("sub", 0), ("set", 0),
              ("get", 0), ("brif", 2), ("ret", 0)]
    return p


class State:
    def __init__(self):
        self.ip, self.stack, self.local, self.outs = 0, [], [0] * 8, []


def features(p, st):
    """[1, arg, top, second, the local named by arg, depth, outputs, ip]."""
    arg = p[st.ip][1]
    d = len(st.stack)
    return [1, arg, st.stack[-1] if d else 0, st.stack[-2] if d > 1 else 0,
            st.local[arg] if 0 <= arg < 8 else 0, d, len(st.outs), st.ip]


def follow(p, st, x, y):
    """Applies a row as the machine would have written it; False if it could not have."""
    code, arg = p[st.ip]
    nxt = st.ip + 1
    if code in ("push", "get"):
        st.stack.append(x)
    elif code == "set":
        st.stack.pop()
        st.local[arg] = x
    elif code in ("add", "sub", "mul"):
        st.stack.pop()
        st.stack[-1] = x
    elif code == "brif":
        st.stack.pop()
        if y not in (nxt, arg):
            return False
        nxt = y
    elif code == "out":
        st.stack.pop()
        st.outs.append(x)
    st.ip = nxt
    return 0 <= nxt < len(p)


def machine(p):
    """The machine's trace as (features, code, x, y) per step."""
    st, rows = State(), []
    while True:
        code, arg = p[st.ip]
        f, d = features(p, st), len(st.stack)
        x = y = 0
        if code == "push":
            x, y = arg, d + 1
        elif code == "get":
            x, y = st.local[arg], arg
        elif code == "set":
            x, y = st.stack[-1], arg
        elif code in ("add", "sub", "mul"):
            a, b = st.stack[-2], st.stack[-1]
            x, y = {"add": a + b, "sub": a - b, "mul": a * b}[code], d - 1
        elif code == "brif":
            x = st.stack[-1]
            y = arg if x != 0 else st.ip + 1
        elif code == "out":
            x, y = st.stack[-1], len(st.outs) + 1
        rows.append((f, code, x, y))
        if code == "ret":
            return rows
        follow(p, st, x, y)


def tokens(rows):
    return [t for f, code, x, y in rows for t in (f[7], CODES.index(code), x, y)]


# Digests: chapter 232's claim and chapter 245's row set.
def le32(values):
    return b"".join(struct.pack("<I", v & 0xFFFFFFFF) for v in values)


def words(*values):
    return b"".join(struct.pack("<q", v) for v in values)


def claim_key(p):
    rows = machine(p)
    prog = hashlib.sha256(b"".join(bytes([CODES.index(c)]) + le32([a]) for c, a in p)).digest()
    trace = hashlib.sha256(le32(tokens(rows))).digest()
    outs = hashlib.sha256(le32([x for f, c, x, y in rows if c == "out"])).digest()
    return hashlib.sha256(words(1) + prog + trace + outs + words(len(rows))).digest()


def set_digest(admitted):
    body = words(len(admitted))
    for group, time, role, key in admitted:
        body += words(1, group, time, role, 1, 4) + key
    return hashlib.sha256(body).digest()


# Students. A feature vector is a list of (index, value) pairs.
def pairs(kind, f, exact=False):
    if kind == "table":
        return [(0, 1)] + [(1 + (j - 1) * (2 * SPAN + 1) + f[j] + SPAN, 1) for j in range(1, 8) if -SPAN <= f[j] <= SPAN]
    v = [Fraction(f[j], 16) if exact else f[j] / 16 for j in range(8)]
    out = list(enumerate(v))
    if kind == "gated":
        out += [(8 + j, v[j] if f[2] != 0 else 0) for j in range(8)]
    return out


def dot(a, b):
    db = dict(b)
    return sum((v * db[i] for i, v in a if i in db), Fraction(0))


def solve(m, rhs):
    """Gauss-Jordan on a nonsingular rational system."""
    n = len(m)
    a = [row[:] + [rhs[i]] for i, row in enumerate(m)]
    for c in range(n):
        r = next(r for r in range(c, n) if a[r][c] != 0)
        a[c], a[r] = a[r], a[c]
        for r in range(n):
            if r != c and a[r][c] != 0:
                k = a[r][c] / a[c][c]
                a[r] = [x - k * y for x, y in zip(a[r], a[c])]
    return [a[i][n] / a[i][i] for i in range(n)]


def least_squares(kind, mine, target):
    """The minimum-norm least-squares weights: where gradient descent from zero converges."""
    rows = [pairs(kind, e[0], True) for e in mine]
    y = [Fraction(e[2 + target]) for e in mine]
    k = [[dot(a, b) for b in rows] for a in rows]
    # Independent rows of the features, found by elimination on their Gram rows.
    basis, reduced = [], []
    for i in range(len(rows)):
        v = k[i][:]
        for bv in reduced:
            piv = next(c for c in range(len(rows)) if bv[c] != 0)
            if v[piv] != 0:
                f = v[piv] / bv[piv]
                v = [x - f * z for x, z in zip(v, bv)]
        if any(v):
            basis.append(i)
            reduced.append(v)
    kb = [k[i] for i in basis]
    normal = [[sum(a[r] * b[r] for r in range(len(rows))) for b in kb] for a in kb]
    coef = solve(normal, [sum(a[r] * y[r] for r in range(len(rows))) for a in kb])
    w = {}
    for c, i in zip(coef, basis):
        for idx, v in rows[i]:
            w[idx] = w.get(idx, 0) + c * v
    return w


def gradient_descent(kind, mine, target, epochs):
    """Binary64 descent from zero on the mean squared error, its gradient as 2 (G w - b)."""
    width = WIDTH[kind]
    rows = [pairs(kind, e[0]) for e in mine]
    m = len(rows)
    g = [[0.0] * width for _ in range(width)]
    b = [0.0] * width
    norm = 0.0
    for r, e in zip(rows, mine):
        for i, v in r:
            norm += v * v
            b[i] += v * e[2 + target] / m
            for j, u in r:
                g[i][j] += v * u / m
    rate = m / norm
    w = [0.0] * width
    for _ in range(epochs):
        w = [wi - rate * 2 * (sum(gij * wj for gij, wj in zip(gi, w)) - bi) for wi, gi, bi in zip(w, g, b)]
    return dict(enumerate(w))


def train(kind, examples, epochs, converged):
    models = {}
    for c in CODES:
        mine = [e for e in examples if e[1] == c]
        for t in range(2):
            if mine:
                models[c, t] = least_squares(kind, mine, t) if converged else gradient_descent(kind, mine, t, epochs)
    return models


def predict(kind, models, code, target, f, exact):
    w = models.get((code, target), {})
    return sum((w.get(i, 0) * v for i, v in pairs(kind, f, exact)), Fraction(0) if exact else 0.0)


def token(v):
    return math.floor(v + Fraction(1, 2)) if isinstance(v, Fraction) else math.floor(v + 0.5)


def score(kind, models, p, exact):
    """The first token where the student's own run differs from the machine's, or -1."""
    want = tokens(machine(p))
    st, got = State(), []
    while len(got) < len(want):
        code = p[st.ip][0]
        f = features(p, st)
        x = token(predict(kind, models, code, 0, f, exact))
        y = token(predict(kind, models, code, 1, f, exact))
        got += [st.ip, CODES.index(code), x, y]
        if code == "ret" or not follow(p, st, x, y):
            break
    for i, (a, b) in enumerate(zip(got, want)):
        if a != b:
            return i, a, b, len(want)
    assert len(got) == len(want)
    return -1, 0, 0, len(want)


def r4(v):
    return math.floor(v * 10000 + 0.5) / 10000


def student(kind, epochs, converged, examples, out):
    models = train(kind, examples, epochs, converged)
    loss, right = 0, 0
    for f, code, x, y in examples:
        for t, want in ((0, x), (1, y)):
            p = predict(kind, models, code, t, f, converged)
            loss += (p - want) ** 2
            right += token(p) == want
    out.append(f"{kind}, {epochs} epochs: training loss {r4(float(loss) / (2 * len(examples)))}, "
               f"{right} of {2 * len(examples)} tokens right from true states")
    exact = 0
    for family in FAMILIES:
        parts = []
        for n in TESTS:
            p = program(family, n)
            first, wrote, want, total = score(kind, models, p, converged)
            exact += first < 0
            parts.append(f"n={n} exact" if first < 0 else f"n={n} token {first} of {total}")
        out.append(f"  {family}: " + ", ".join(parts))
        if first >= 0:
            step = first // 4
            out.append(f"    first miss at n=32: step {step} ({machine(p)[step][1]}) wrote {wrote} for "
                       f"{FIELDS[first % 4]}, where the machine has {want}")
    return exact


def example():
    out = ["chapter 224's machine; four program families, each counting n down to 1 in local 0:",
           "  count prints n, ..., 1; sum prints 1 + ... + n; stride prints 2n, 2n-2, ..., 2; squares prints n*n, ..., 1",
           "training traces: count and sum at n = 1, 2, 3, 4, each replayed exactly and admitted as train rows"]
    examples, admitted, time = [], [], 0
    for fi, family in enumerate(FAMILIES[:2]):
        for n in (1, 2, 3, 4):
            p = program(family, n)
            examples += machine(p)
            admitted.append((fi * 1000 + n, time, 1, claim_key(p)))
            time += 1
    out.append(f"  {len(examples)} rows of the machine, {4 * len(examples)} tokens")
    # Replay reaches step 3's out and finds a different x there.
    out.append("  count 4 again with step 3's x changed and its digest redone: failed")
    held = 0
    for fi, family in enumerate(FAMILIES):
        for n in TESTS:
            if fi < 2 and n == 4:
                continue
            admitted.append((fi * 1000 + n, 8 + held, 4, claim_key(program(family, n))))
            held += 1
    out.append(f"held out: count and sum at n = 8, 16, 32, stride and squares at n = 4, 8, 16, 32: {held} heldout rows, "
               "never trained on")
    out.append(f"  row set {set_digest(admitted)[:6].hex()}: {len(admitted)} rows, {len(admitted) - held} train, "
               f"{held} heldout")
    out += ["students: one affine model per instruction and token, trained from zero by full-batch gradient descent;",
            "scored by rollout alone: the state is folded from the student's own rows, chapter 232 replays the run,",
            "and a miss is the first token that differs, of the machine's"]
    tally = [student("linear", 10000, True, examples, out), student("gated", 10000, False, examples, out),
             student("gated", 100000, True, examples, out), student("table", 10000, True, examples, out)]
    out.append(f"exact runs of 16: linear {tally[0]}, gated after 10000 epochs {tally[1]}, after 100000 {tally[2]}, "
               f"table {tally[3]}")
    return out


if __name__ == "__main__":
    out = example()
    fixture = (ROOT / "psionic/fixtures/students.out").read_text().splitlines()
    for n, (a, b) in enumerate(zip(out, fixture)):
        assert a == b, f"line {n + 1}:\n  oracle  {a}\n  fixture {b}"
    assert len(out) == len(fixture), (len(out), len(fixture))
    print("students oracle: three students as exact least-squares limits, one by binary64 descent; "
          f"the example's {len(out)} lines are rebuilt line for line")
