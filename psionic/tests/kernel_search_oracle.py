"""Independent oracle for chapter 188 (an agent optimizes a Psionic kernel).

Standard library only, sharing no code with the Coax units or with
kernel_search_fixture.py. It rebuilds psionic/fixtures/kernel-search.out line
for line:
- the CPU lines, by regenerating chapter 188's ten cases (its own xorshift),
  rerunning the arithmetic of chapter 30's kernels, the tuner's choice and the
  agent's winner in binary64 with one rounding to binary32, and applying the
  contract (one f32 step of the f64 norm) itself;
- the L4 lines, by parsing the raw evaluator ledger in
  psionic/kernels/norm/ledger/ and redoing every median, choice and ratio.
"""
import math
import pathlib
import struct

ROOT = pathlib.Path(__file__).resolve().parents[2]
LEDGER = ROOT / "psionic/kernels/norm/ledger"
BLOCKS = [32, 64, 128, 256, 512, 1024]
SHAPES = [(86, 4096), (1, 4096), (7, 5120), (3, 1000), (5, 4097), (2, 1), (1, 17), (4, 4096), (2, 4096), (3, 4096)]
NAMES = ["the workload", "a decode row", "Mammoth's width 5120", "width 1000", "width 4097", "width 1", "width 17",
         "extreme values", "zero rows", "one huge value among tiny ones"]
AGENT_DIR = "psionic/kernels/norm/agent/"
M32 = 0xFFFFFFFF


def f32(v):
    return struct.unpack("<f", struct.pack("<f", v))[0]


def from_bits(b):
    return struct.unpack("<f", struct.pack("<I", b))[0]


def bits(v):
    return struct.unpack("<I", struct.pack("<f", v))[0]


EPS = f32(1e-6)


def xorshift(s):
    s ^= (s << 13) & M32
    s ^= s >> 17
    s ^= (s << 5) & M32
    return s


def case(c):
    rows, width = SHAPES[c]
    state = ((c + 1) * 2654435761) & M32
    extremes = [f32(3.4028235e38), -f32(3.4028235e38), from_bits(1), 0.0, 1.0, f32(-1e-20), 65504.0, -from_bits(0x7FFFFF)]
    x = []
    for i in range(rows * width):
        state = xorshift(state)
        v = ((state >> 8) - (1 << 23)) / (1 << 21)
        if c == 7:
            v = extremes[i % 8]
        if c == 8:
            v = 0.0
        if c == 9:
            v = f32(1e30) if i % width == (i // width) * 97 else f32(1e-30)
        x.append(v)
    scale = []
    for i in range(width):
        state = xorshift(state)
        scale.append(((state >> 8) - (1 << 23)) / (1 << 24))
    return rows, width, x, scale


def ok(rows, width, x, scale, out):
    """The contract: each output within one f32 step (above its f32 rounding) of the f64 norm."""
    for r in range(rows):
        at = r * width
        total = 0.0
        for i in range(width):
            total += x[at + i] * x[at + i]
        d = math.sqrt(total / width + EPS)
        for i in range(width):
            want = x[at + i] / d * (1 + scale[i])
            a = abs(f32(want))
            ulp = from_bits(bits(a) + 1) - a
            if not abs(out[at + i] - want) <= ulp:
                return False
    return True


def ch30(rows, width, x, scale):
    n = width // 16
    out = []
    for r in range(rows):
        parts = []
        for k in range(16):
            s = 0.0
            for i in range(n):
                v = x[r * width + k * n + i]
                s += v * v
            parts.append(s)
        sq = 0.0
        for p in parts:
            sq += p
        d = math.sqrt(sq / width + EPS)
        out += [f32(x[r * width + i] / d * (1 + scale[i])) for i in range(width)]
    return out


def tuned(rows, width, x, scale):
    c = 32
    per = (width + c - 1) // c
    out = []
    for r in range(rows):
        parts = []
        for j in range(c):
            s = 0.0
            for i in range(j * per, min(j * per + per, width)):
                v = x[r * width + i]
                s += v * v
            parts.append(s)
        sq = 0.0
        for p in parts:
            sq += p
        d = math.sqrt(sq / width + EPS)
        out += [f32(x[r * width + i] / d * (1 + scale[i])) for i in range(width)]
    return out


def winner(rows, width, x, scale):
    c = min(64, width)
    per = (width + c - 1) // c
    out = []
    for r in range(rows):
        parts = []
        for j in range(c):
            lo = min(j * per, width)
            n = min(per, width - lo)
            s0 = s1 = 0.0
            for k in range(n // 2):
                a, b = x[r * width + lo + 2 * k], x[r * width + lo + 2 * k + 1]
                s0 += a * a
                s1 += b * b
            if n % 2:
                v = x[r * width + lo + n - 1]
                s0 += v * v
            parts.append(s0 + s1)
        s0 = s1 = 0.0
        for j in range(c // 4):
            a, b, d, e = parts[4 * j:4 * j + 4]
            s0 += a
            s1 += b
            s0 += d
            s1 += e
        for j in range(c - c % 4, c):
            s0 += parts[j]
        inv = 1 / math.sqrt((s0 + s1) / width + EPS)
        out += [f32(x[r * width + i] * inv * (1 + scale[i])) for i in range(width)]
    return out


def r2(v):
    return math.floor(v * 100 + 0.5) / 100


def us(s):
    return r2(s * 1e6)


def num(v):
    """Coax prints a whole f64 with one decimal and others in shortest form."""
    return repr(v)


def parse(path):
    run = {"knobs": [], "cpu": {}, "device": {}, "time": {}, "pass": {}, "note": "", "confirm": False}
    for line in path.read_text().splitlines():
        p = line.split()
        if not p:
            continue
        if p[0] == "source":
            run["file"], run["sha"] = p[1], p[3]
        elif p[0] == "prefix":
            run["prefix"] = p[1]
        elif p[0] == "commit":
            run["commit"] = p[1]
        elif p[0] == "confirmation":
            run["confirm"] = True
        elif p[0] == "gpu":
            run["gpu"] = " ".join(p[1:])
        elif p[0] == "seconds":
            run["seconds"] = int(p[1])
        elif p[0] == "error" and "CUDA error" in line:
            run["note"] = line[line.index("CUDA error"):]
        elif p[0] in ("cpu", "device") and len(p) == 4:
            k = int(p[1])
            if k not in run["knobs"]:
                run["knobs"].append(k)
            run[p[0]].setdefault(k, {})[int(p[2])] = p[3]
        elif p[0] == "time":
            run["time"][(int(p[1]), int(p[2]))] = sorted(float(v) for v in p[3:])
        elif p[0] == "pass":
            run["pass"][int(p[1])] = (int(p[2]), float(p[3]), float(p[4]))
    return run


def configs(run, declared):
    """(knob, block, median, low, high, passed) in the evaluator's order."""
    out = []
    for k in run["knobs"]:
        cases = [c for c in range(10) if not declared or SHAPES[c][1] % 16 == 0]
        good = all(run["cpu"].get(k, {}).get(c) == "ok" and run["device"].get(k, {}).get(c) == "ok" for c in cases)
        for b in BLOCKS:
            t = run["time"].get((k, b))
            if t:
                out.append((k, b, t[len(t) // 2], t[0], t[-1], good, len(t)))
    return out


def best(cs):
    pick = None
    for c in cs:
        if c[5] and (pick is None or c[2] < pick[2]):
            pick = c
    return pick


def show(what, c):
    return f"{what}: knob {c[0]}, block {c[1]}: {num(us(c[2]))} us ({num(us(c[3]))} to {num(us(c[4]))} over {c[6]} trials)"


def describe(k):
    if k == 0:
        return " (one item a row, both passes fused)"
    return f" ({k // 100} {'strided' if k % 10 == 1 else 'contiguous'} chunks, {(k // 10) % 10} outputs an item)"


def split(run, k):
    b, p0, p1 = run["pass"][k]
    return f"; pass 0 {num(us(p0))} us, pass 1 {num(us(p1))} us at block {b}"


def example():
    out = ["workload: Elephant's offset RMS norm over chapter 10's 86-token sample, 86 x 4096, epsilon 1e-6",
           "contract: squares added in f64 in any order, the rest in f64, one rounding to f32;",
           "  each output must be one of the two f32 values next to the f64 norm",
           "held out from the agent: " + ", ".join(NAMES[1:]), "", "on this CPU, now, every case:"]
    cases = [case(c) for c in range(10)]
    for name, fn in (("chapter 30 (n30)", ch30), ("the tuner's choice (nt 3240)", tuned), ("the agent's winner (a8 64160)", winner)):
        failed = [c for c in range(10) if not ok(*cases[c], fn(*cases[c]))]
        line = f"  {name}: {10 - len(failed)} of 10 cases"
        if failed:
            line += "; wrong on " + ", ".join(NAMES[c] for c in failed)
        out.append(line)
    runs = [parse(p) for p in sorted(LEDGER.glob("*.log"))]
    base = max(i for i, r in enumerate(runs) if r["prefix"] == "n30")
    tuner = max(i for i, r in enumerate(runs) if r["prefix"] == "nt")
    out += ["", f"on the L4 ({runs[base]['gpu']}), from the ledger of {len(runs)} evaluations:"]
    b = best(configs(runs[base], True))
    out.append(f"  chapter 30, evaluation {base + 1}, on its declared widths: {show('best', b)}{split(runs[base], b[0])}")
    tc = configs(runs[tuner], False)
    t = best(tc)
    passing = sum(1 for k in runs[tuner]["knobs"] if all(runs[tuner]["cpu"][k].get(c) == "ok" and runs[tuner]["device"][k].get(c) == "ok" for c in range(10)))
    out.append(f"  the tuner, evaluation {tuner + 1}: {len(runs[tuner]['knobs'])} knobs x 6 blocks = {len(tc)} configurations, "
               f"{passing} knobs passing every case on both")
    out.append(f"    {show('best', t)}{describe(t[0])}, {num(r2(b[2] / t[2]))} x chapter 30{split(runs[tuner], t[0])}")
    slow = tc[0]
    for c in tc:
        if c[2] > slow[2]:
            slow = c
    out.append(f"    {show('slowest', slow)}{describe(slow[0])}")
    agent = [i for i, r in enumerate(runs) if r["file"].startswith(AGENT_DIR) and not r["confirm"]]
    out.append(f"  the agent: {len(agent)} budgeted evaluations, failures included")
    win = None
    for i in agent:
        r = runs[i]
        cs = configs(r, False)
        line = f"    {r['prefix']}, evaluation {i + 1}: {len(r['knobs'])} knobs"
        if r["note"]:
            blocks = sum(1 for c in cs if c[0] == cs[0][0])
            out.append(line + f", stopped at block {BLOCKS[blocks]}: {r['note']}")
            continue
        good = sum(1 for k in r["knobs"] if all(r["cpu"][k].get(c) == "ok" and r["device"][k].get(c) == "ok" for c in range(10)))
        c = best(cs)
        faster = c[4] < t[3]
        out.append(line + f", {good} passing; {show('best', c)}, {'faster' if faster else 'not faster'} than the tuner")
        if faster and (win is None or c[2] < win[1][2]):
            win = (i, c)
    i, w = win
    confirm = max(j for j, r in enumerate(runs) if r["prefix"] == runs[i]["prefix"])
    r = runs[confirm]
    c = next(x for x in configs(r, False) if x[0] == w[0] and x[1] == w[1])
    out.append(f"  winner: {runs[i]['prefix']}, re-measured in evaluation {confirm + 1}, beside evaluations {tuner + 1} and {base + 1}:")
    out.append(f"    {show('same', c)}, {num(r2(t[2] / c[2]))} x the tuner, {num(r2(b[2] / c[2]))} x chapter 30{split(r, c[0])}")
    out.append(f"    {int(math.floor(3 * 352256 * 4 / c[2] / 1e9 + 0.5))} GB/s effective for the 4.23 MB a call must move "
               "(x read twice, out written)")
    out.append(f"    source {r['file']}")
    out.append(f"    sha256 {r['sha']}, at commit {r['commit']}")
    total = sum(x["seconds"] for x in runs)
    spent = sum(runs[j]["seconds"] for j in agent)
    out.append(f"  evaluator time, admission to last trial: {total} s in all, {spent} s of it the agent's {len(agent)}")
    return out


if __name__ == "__main__":
    out = example()
    fixture = (ROOT / "psionic/fixtures/kernel-search.out").read_text().splitlines()
    for n, (a, b) in enumerate(zip(out, fixture)):
        assert a == b, f"line {n + 1}:\n  oracle  {a}\n  fixture {b}"
    assert len(out) == len(fixture), (len(out), len(fixture))
    print(f"kernel-search oracle: ten cases regenerated and three kernels rerun; the ledger re-read; "
          f"the example's {len(out)} lines are rebuilt line for line")
