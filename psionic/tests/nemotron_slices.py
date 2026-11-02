"""Chapter 163's original-weight slices: an independent binary64 oracle in
plain Python, written from the published NemotronH architecture, and the
comparison with the Coax slices.

    python3 -I psionic/tests/nemotron_slices.py ranges MODEL_DIRECTORY OUT_DIR
    python3 -I psionic/tests/nemotron_slices.py compare MODEL_DIRECTORY OUT_DIR

ranges reads the safetensors header (JSON only) and writes each tensor's byte
range and the prompt's token ids. compare recomputes every slice from the
weights and checks the Coax outputs within the frozen tolerance. Nothing from
the download is imported or run. Tooling only.
"""
import array
import json
import math
import operator
import pathlib
import struct
import sys

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


if __name__ == "__main__":
    sys.exit(main())
