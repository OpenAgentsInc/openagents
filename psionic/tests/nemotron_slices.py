"""Chapter 163's original-weight slices: an independent binary64 oracle in
plain Python, written from the published NemotronH architecture, and the
comparison with the Coax slices.

    python3 -I psionic/tests/nemotron_slices.py ranges MODEL_DIRECTORY OUT_DIR
    python3 -I psionic/tests/nemotron_slices.py compare MODEL_DIRECTORY OUT_DIR
    python3 -I psionic/tests/nemotron_slices.py model MODEL_DIRECTORY OUT_DIR

model recomputes the whole 42-layer model in binary64 over the prompt and the
greedy tokens the Coax run chose (psionic/tests/nemotron_run.c writes them),
on every core, and compares every layer's hidden rows for the prompt, the
prompt's logits and the last step's logits, and every greedy choice.

ranges reads the safetensors header (JSON only) and writes each tensor's byte
range and the prompt's token ids. compare recomputes every slice from the
weights and checks the Coax outputs within the frozen tolerance. Nothing from
the download is imported or run. Tooling only.
"""
import array
import json
import math
import multiprocessing
import operator
import pathlib
import struct
import sys

# The whole model, frozen before its first comparison: every layer's rows
# and the logits may differ by at most MODEL_TOLERANCE * (1 + the largest
# |oracle value| of that layer or of those logits).
MODEL_TOLERANCE = 1e-3
# Profile nemotron-h-cpu-f64-v1 on original weights, frozen before the first
# comparison: every value may differ from this oracle by at most
# TOLERANCE * (1 + the largest |oracle value| of that slice).
TOLERANCE = 1e-4
H, V, C, HEADS, P, N, GROUPS = 3136, 7680, 9728, 96, 80, 128, 8
EPS = 1e-5
READOUT = 8192
# "The capital of France is" after the beginning-of-text token <s>. Each word
# is one vocabulary entry (the pinned tokenizer.json checks "ignore_merges",
# so a pre-token that is a whole entry is that entry); compare looks each up.
PROMPT = ["<s>", "The", "Ġcapital", "Ġof", "ĠFrance", "Ġis"]


def header(directory):
    path = pathlib.Path(directory) / "model.safetensors"
    with path.open("rb") as f:
        n = struct.unpack("<Q", f.read(8))[0]
        if not 2 <= n <= 1 << 20:
            raise SystemExit(f"refused: header of {n} bytes")
        return path, 8 + n, json.loads(f.read(n))


def tokens(directory):
    vocab = json.loads((pathlib.Path(directory) / "tokenizer.json").read_text())["model"]["vocab"]
    return [vocab[w] for w in PROMPT]


def bf16(f, begin, count):
    f.seek(begin)
    raw = f.read(2 * count)
    wide = bytearray(4 * count)
    wide[2::4] = raw[0::2]
    wide[3::4] = raw[1::2]
    return array.array("f", bytes(wide))


class Weights:
    def __init__(self, directory):
        self.path, self.base, self.header = header(directory)
        self.f = self.path.open("rb")

    def get(self, name, rows=None):
        t = self.header[name]
        count = 1
        for d in t["shape"]:
            count *= d
        if rows is not None:
            count = rows * t["shape"][1]
        return bf16(self.f, self.base + t["data_offsets"][0], count)

    def row(self, name, index, width):
        t = self.header[name]
        return bf16(self.f, self.base + t["data_offsets"][0] + 2 * index * width, width)


def rms(x, w):
    r = math.sqrt(sum(v * v for v in x) / len(x) + EPS)
    return [v / r * w[i] for i, v in enumerate(x)]


def matvec(w, x, rows):
    cols = len(x)
    return [sum(map(operator.mul, w[r * cols:(r + 1) * cols], x)) for r in range(rows)]


def silu(x):
    return x / (1 + math.exp(-x)) if x > -700 else 0.0


def softplus(x):
    return x + math.log1p(math.exp(-x)) if x > 0 else math.log1p(math.exp(x))


def mamba(wt, layer, hidden):
    p = f"backbone.layers.{layer}."
    norm, inp = wt.get(p + "norm.weight"), wt.get(p + "mixer.in_proj.weight")
    conv, cb = wt.get(p + "mixer.conv1d.weight"), wt.get(p + "mixer.conv1d.bias")
    alog, dtb, dd = wt.get(p + "mixer.A_log"), wt.get(p + "mixer.dt_bias"), wt.get(p + "mixer.D")
    gn, outp = wt.get(p + "mixer.norm.weight"), wt.get(p + "mixer.out_proj.weight")
    raw, out = [], []
    state = [0.0] * (HEADS * P * N)
    for t, h in enumerate(hidden):
        proj = matvec(inp, rms(h, norm), V + C + HEADS)
        z, xbc, dt = proj[:V], proj[V:V + C], proj[V + C:]
        raw.append(xbc)
        cv = []
        for ch in range(C):
            s = cb[ch]
            for back in range(4):
                if t >= back:
                    s += raw[t - back][ch] * conv[ch * 4 + 3 - back]
            cv.append(silu(s))
        y = [0.0] * V
        for head in range(HEADS):
            g = head // (HEADS // GROUPS)
            step = softplus(dt[head] + dtb[head])
            keep = math.exp(-math.exp(alog[head]) * step)
            B = cv[V + g * N:V + (g + 1) * N]
            Cc = cv[V + GROUPS * N + g * N:V + GROUPS * N + (g + 1) * N]
            for q in range(P):
                x = cv[head * P + q]
                at = (head * P + q) * N
                u = step * x
                row = [keep * state[at + k] + u * B[k] for k in range(N)]
                state[at:at + N] = row
                yy = sum(map(operator.mul, row, Cc)) + dd[head] * x
                zz = z[head * P + q]
                y[head * P + q] = yy * silu(zz)
        w = V // GROUPS
        gated = []
        for g in range(GROUPS):
            gated += rms(y[g * w:(g + 1) * w], gn[g * w:(g + 1) * w])
        upd = matvec(outp, gated, H)
        out.append([a + b for a, b in zip(h, upd)])
    return out, state


def mlp(wt, layer, hidden):
    p = f"backbone.layers.{layer}."
    norm, up, down = wt.get(p + "norm.weight"), wt.get(p + "mixer.up_proj.weight"), wt.get(p + "mixer.down_proj.weight")
    out = []
    for h in hidden:
        a = [max(v, 0.0) ** 2 for v in matvec(up, rms(h, norm), 12544)]
        out.append([x + y for x, y in zip(h, matvec(down, a, H))])
    return out


def attention(wt, layer, hidden):
    p = f"backbone.layers.{layer}."
    norm = wt.get(p + "norm.weight")
    wq, wk, wv, wo = (wt.get(p + f"mixer.{n}_proj.weight") for n in "qkvo")
    keys, values, out = [], [], []
    for h in hidden:
        x = rms(h, norm)
        q = matvec(wq, x, 5120)
        keys.append(matvec(wk, x, 1024))
        values.append(matvec(wv, x, 1024))
        mixed = []
        for head in range(40):
            kv = (head // 5) * 128
            qh = q[head * 128:(head + 1) * 128]
            scores = [sum(map(operator.mul, qh, k[kv:kv + 128])) / math.sqrt(128) for k in keys]
            top = max(scores)
            e = [math.exp(s - top) for s in scores]
            total = sum(e)
            mixed += [sum(e[j] * values[j][kv + d] for j in range(len(keys))) / total for d in range(128)]
        out.append([a + b for a, b in zip(h, matvec(wo, mixed, H))])
    return out


def load(out, name, count):
    a = array.array("f")
    with (pathlib.Path(out) / name).open("rb") as f:
        a.frombytes(f.read())
    if len(a) != count:
        raise SystemExit(f"{name}: {len(a)} values, expected {count}")
    return a


def gap(name, got, ref):
    flat = [v for r in ref for v in r] if isinstance(ref[0], list) else list(ref)
    worst = max(abs(a - b) for a, b in zip(got, flat))
    limit = TOLERANCE * (1 + max(abs(v) for v in flat))
    print(f"{name}: largest difference {worst:.3g}, limit {limit:.3g}, {'within' if worst <= limit else 'OUTSIDE'}")
    return worst <= limit


def main():
    if len(sys.argv) == 4 and sys.argv[1] == "model":
        return whole(sys.argv[2], sys.argv[3])
    if len(sys.argv) != 4 or sys.argv[1] not in ("ranges", "compare"):
        print(__doc__.strip().splitlines()[3].strip(), file=sys.stderr)
        return 2
    directory, out = sys.argv[2], pathlib.Path(sys.argv[3])
    ids = tokens(directory)
    if sys.argv[1] == "ranges":
        path, base, head = header(directory)
        lines = [f"{k} {base + v['data_offsets'][0]} {base + v['data_offsets'][1]}" for k, v in sorted(head.items()) if k != "__metadata__"]
        (out / "ranges.txt").write_text("\n".join(lines) + "\n")
        (out / "tokens.txt").write_text(" ".join(map(str, ids)) + "\n")
        print("prompt tokens", ids)
        return 0
    wt = Weights(directory)
    n = len(ids)
    embed = [list(wt.row("backbone.embeddings.weight", t, H)) for t in ids]
    ok = gap("embedding rows (exact)", load(out, "embed.f32", n * H), embed)
    ok = ok and list(load(out, "embed.f32", n * H)) == [v for r in embed for v in r]
    m, state = mamba(wt, 0, embed)
    ok = gap("mamba layer 0", load(out, "mamba.f32", n * H), m) and ok
    ok = gap("mamba layer 0 carried state", load(out, "mamba-state.f32", HEADS * P * N), state) and ok
    ok = gap("mamba layer 0 one token at a time", load(out, "mamba-step.f32", n * H), m) and ok
    f = mlp(wt, 1, m)
    ok = gap("mlp layer 1", load(out, "mlp.f32", n * H), f) and ok
    a = attention(wt, 12, f)
    ok = gap("attention layer 12", load(out, "attention.f32", n * H), a) and ok
    x = rms(a[-1], wt.get("backbone.norm_f.weight"))
    logits = matvec(wt.get("lm_head.weight", rows=READOUT), x, READOUT)
    got = load(out, "readout.f32", READOUT)
    ok = gap("readout rows 0..8191", got, logits) and ok
    best = max(range(READOUT), key=lambda i: (logits[i], -i))
    print(f"largest logit in rows 0..8191: oracle {best}, Coax {max(range(READOUT), key=lambda i: (got[i], -i))}")
    return 0 if ok else 1


# ---- the whole model, on every core ---------------------------------------

_W = None


def _open(directory):
    global _W
    _W = Weights(directory)


def _rows(name, cols, r0, r1, xs):
    t = _W.header[name]
    w = bf16(_W.f, _W.base + t["data_offsets"][0] + 2 * r0 * cols, (r1 - r0) * cols)
    return [[sum(map(operator.mul, w[r * cols:(r + 1) * cols], x)) for x in xs] for r in range(r1 - r0)]


def pmatvec(pool, name, rows, xs):
    """Every x in xs times the named rows x len(x) matrix, rows split across workers."""
    cols = len(xs[0])
    step = max(1, rows // 208)
    parts = pool.starmap(_rows, [(name, cols, r, min(rows, r + step), xs) for r in range(0, rows, step)])
    out = [[0.0] * rows for _ in xs]
    r = 0
    for part in parts:
        for row in part:
            for t, v in enumerate(row):
                out[t][r] = v
            r += 1
    return out


def _scan(head, dts, keep, xs, bs, cs, d, zs):
    state = [[0.0] * N for _ in range(P)]
    ys = []
    for t in range(len(dts)):
        y = []
        for q in range(P):
            u = dts[t] * xs[t][q]
            row = [keep[t] * state[q][k] + u * bs[t][k] for k in range(N)]
            state[q] = row
            yy = sum(map(operator.mul, row, cs[t])) + d * xs[t][q]
            y.append(yy * silu(zs[t][q]))
        ys.append(y)
    return ys


def model_mamba(pool, wt, layer, hidden):
    p = f"backbone.layers.{layer}."
    norm, conv, cb = wt.get(p + "norm.weight"), wt.get(p + "mixer.conv1d.weight"), wt.get(p + "mixer.conv1d.bias")
    alog, dtb, dd, gn = wt.get(p + "mixer.A_log"), wt.get(p + "mixer.dt_bias"), wt.get(p + "mixer.D"), wt.get(p + "mixer.norm.weight")
    proj = pmatvec(pool, p + "mixer.in_proj.weight", V + C + HEADS, [rms(h, norm) for h in hidden])
    T = len(hidden)
    raw = [pr[V:V + C] for pr in proj]
    cv = []
    for t in range(T):
        row = []
        for ch in range(C):
            s = cb[ch]
            for back in range(4):
                if t >= back:
                    s += raw[t - back][ch] * conv[ch * 4 + 3 - back]
            row.append(silu(s))
        cv.append(row)
    jobs = []
    for head in range(HEADS):
        g = head // (HEADS // GROUPS)
        dts = [softplus(proj[t][V + C + head] + dtb[head]) for t in range(T)]
        keep = [math.exp(-math.exp(alog[head]) * dt) for dt in dts]
        jobs.append((head, dts, keep, [cv[t][head * P:(head + 1) * P] for t in range(T)],
                     [cv[t][V + g * N:V + (g + 1) * N] for t in range(T)],
                     [cv[t][V + GROUPS * N + g * N:V + GROUPS * N + (g + 1) * N] for t in range(T)],
                     dd[head], [proj[t][head * P:(head + 1) * P] for t in range(T)]))
    heads = pool.starmap(_scan, jobs)
    w = V // GROUPS
    gated = []
    for t in range(T):
        y = [v for head in range(HEADS) for v in heads[head][t]]
        gt = []
        for g in range(GROUPS):
            gt += rms(y[g * w:(g + 1) * w], gn[g * w:(g + 1) * w])
        gated.append(gt)
    upd = pmatvec(pool, p + "mixer.out_proj.weight", H, gated)
    return [[a + b for a, b in zip(h, u)] for h, u in zip(hidden, upd)]


def model_mlp(pool, wt, layer, hidden):
    p = f"backbone.layers.{layer}."
    norm = wt.get(p + "norm.weight")
    up = pmatvec(pool, p + "mixer.up_proj.weight", 12544, [rms(h, norm) for h in hidden])
    down = pmatvec(pool, p + "mixer.down_proj.weight", H, [[max(v, 0.0) ** 2 for v in u] for u in up])
    return [[a + b for a, b in zip(h, u)] for h, u in zip(hidden, down)]


def model_attention(pool, wt, layer, hidden):
    p = f"backbone.layers.{layer}."
    xs = [rms(h, wt.get(p + "norm.weight")) for h in hidden]
    q = pmatvec(pool, p + "mixer.q_proj.weight", 5120, xs)
    keys = pmatvec(pool, p + "mixer.k_proj.weight", 1024, xs)
    values = pmatvec(pool, p + "mixer.v_proj.weight", 1024, xs)
    mixed = []
    for t in range(len(hidden)):
        m = []
        for head in range(40):
            kv = (head // 5) * 128
            qh = q[t][head * 128:(head + 1) * 128]
            scores = [sum(map(operator.mul, qh, keys[j][kv:kv + 128])) / math.sqrt(128) for j in range(t + 1)]
            top = max(scores)
            e = [math.exp(x - top) for x in scores]
            total = sum(e)
            m += [sum(e[j] * values[j][kv + d] for j in range(t + 1)) / total for d in range(128)]
        mixed.append(m)
    o = pmatvec(pool, p + "mixer.o_proj.weight", H, mixed)
    return [[a + b for a, b in zip(h, u)] for h, u in zip(hidden, o)]


PATTERN = "M-M-M-MM-M-M*-M-M*-M-M-M*-M-M-MM*-MMM-M-M-"


def whole(directory, out):
    lines = (pathlib.Path(out) / "run-tokens.txt").read_text().split("\n")
    prompt, chosen = list(map(int, lines[0].split())), list(map(int, lines[1].split()))
    seq = prompt + chosen[:-1]
    n, T = len(prompt), len(seq)
    wt = Weights(directory)
    trace = load(out, "trace.f32", 42 * n * H)
    ok = True
    with multiprocessing.Pool(initializer=_open, initargs=(directory,)) as pool:
        hidden = [list(wt.row("backbone.embeddings.weight", t, H)) for t in seq]
        worst = 0.0
        for layer, kind in enumerate(PATTERN):
            f = model_mamba if kind == "M" else model_attention if kind == "*" else model_mlp
            hidden = f(pool, wt, layer, hidden)
            got = trace[layer * n * H:(layer + 1) * n * H]
            ref = [v for r in hidden[:n] for v in r]
            d = max(abs(a - b) for a, b in zip(got, ref))
            limit = MODEL_TOLERANCE * (1 + max(abs(v) for v in ref))
            worst = max(worst, d / limit)
            if d > limit:
                print(f"layer {layer} ({kind}): largest difference {d:.3g}, limit {limit:.3g}, OUTSIDE")
                ok = False
        print(f"42 layers, the prompt's rows: largest difference {worst:.3g} of each layer's limit, {'within' if ok else 'OUTSIDE'}")
        norm = wt.get("backbone.norm_f.weight")
        xs = [rms(h, norm) for h in hidden[n - 1:]]
        logits = pmatvec(pool, "lm_head.weight", 131072, xs)
    ok = gap("logits after the prompt", load(out, "logits-prompt.f32", 131072), logits[0]) and ok
    ok = gap("logits after the last step", load(out, "logits-last.f32", 131072), logits[-1]) and ok
    picks = [max(range(131072), key=lambda i: (row[i], -i)) for row in logits]
    same = picks == chosen
    print(f"greedy choices: oracle {picks}, Coax {chosen}, {'same' if same else 'DIFFERENT'}")
    return 0 if ok and same else 1


if __name__ == "__main__":
    sys.exit(main())
