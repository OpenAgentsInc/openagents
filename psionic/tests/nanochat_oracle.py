#!/usr/bin/env python3
"""Independent definition of the pinned nanochat GPT forward pass (pure Python, binary64).

Written from karpathy/nanochat 92d63d4e8bb4df75c3b71618f31ddde2378b2bcd
nanochat/gpt.py (sha256 8cdedc2e...04e3) and nanochat/flash_attention.py
(sha256 a10b0b10...2c2f, the SDPA fallback's mask and single-token slicing),
read and never executed. It builds every parameter by its state_dict name,
runs the training-style full sequence with an explicit causal/window mask,
runs the KV-cache path one token at a time with the fallback's key slicing,
and prints the per-block residuals, logits and loss that
psionic/lib/nanochat/gpt.coax must reproduce.

Profile: binary64 on the CPU. F.rms_norm's eps is None, which torch resolves
to finfo(dtype).eps; for a binary64 input that is 2**-52.
"""
import argparse
import json
import math
import struct

EPS = 2.0 ** -52
BASE = 100000.0
SOFTCAP = 15.0
SOURCE = "karpathy/nanochat@92d63d4e8bb4df75c3b71618f31ddde2378b2bcd nanochat/gpt.py, nanochat/flash_attention.py"


def window_tokens(n):
    return [(3 * i * i + i) % 7 for i in range(n)]


CASES = {
    # One block: value embedding on layer 0, backout at layer 0, GQA 4/2.
    "tiny": dict(sequence_len=8, vocab_size=5, n_layer=1, n_head=4, n_kv_head=2, n_embd=24, window_pattern="L",
                 head=0.5, tokens=[1, 3, 0, 2, 4, 1], rounding=None),
    # Three blocks "SL" -> S, L, L (the last is always L); short window 128 for
    # sequence_len 256, so positions 129.. lose key 0 in layer 0.
    "window": dict(sequence_len=256, vocab_size=7, n_layer=3, n_head=2, n_kv_head=1, n_embd=24, window_pattern="SL",
                   head=0.5, tokens=window_tokens(140), rounding=None),
    # The shipped checkpoint: chapter 123's 320-token table plus nine specials.
    "checkpoint": dict(sequence_len=64, vocab_size=329, n_layer=2, n_head=2, n_kv_head=1, n_embd=32, window_pattern="SSSL",
                       head=0.5, tokens=[328, 72, 101, 108, 108, 111, 32, 256, 300, 7, 319, 0], rounding="nanochat"),
}
WINDOW_LOGIT_POSITIONS = [0, 1, 2, 127, 128, 129, 130, 139]
WINDOW_BLOCK_POSITIONS = [1, 129, 139]

# Per-kind (scale, offset) for the deterministic weights; the head scale is per case.
KINDS = {"resid_lambdas": (0.1, 1.0), "x0_lambdas": (0.2, 0.1), "smear_lambda": (0.2, 0.6), "backout_lambda": (0.1, 0.2),
         "wte": (0.8, 0.0), "c_q": (0.3, 0.0), "c_k": (0.3, 0.0), "c_v": (0.3, 0.0), "attn.c_proj": (0.2, 0.0),
         "ve_gate": (0.4, 0.0), "c_fc": (0.3, 0.0), "mlp.c_proj": (0.2, 0.0), "lm_head": (None, 0.0),
         "smear_gate": (0.5, 0.0), "value_embeds": (0.5, 0.0)}


def has_ve(i, n_layer):
    return i % 2 == (n_layer - 1) % 2


def padded(c):
    return ((c["vocab_size"] + 63) // 64) * 64


def windows(c):
    long = c["sequence_len"]
    short = -(-long // 4 // 128) * 128
    pat = c["window_pattern"].upper()
    assert all(ch in "SL" for ch in pat)
    out = [{"L": long, "S": short}[pat[i % len(pat)]] for i in range(c["n_layer"])]
    out[-1] = long
    return out


def inventory(c):
    """(name, kind, shape) in GPT.state_dict() order."""
    d, hd = c["n_embd"], c["n_embd"] // c["n_head"]
    v, L, kv = padded(c), c["n_layer"], c["n_kv_head"] * (c["n_embd"] // c["n_head"])
    out = [("resid_lambdas", "resid_lambdas", (L,)), ("x0_lambdas", "x0_lambdas", (L,)),
           ("smear_lambda", "smear_lambda", (1,)), ("backout_lambda", "backout_lambda", (1,)),
           ("transformer.wte.weight", "wte", (v, d))]
    for i in range(L):
        p = f"transformer.h.{i}."
        out += [(p + "attn.c_q.weight", "c_q", (c["n_head"] * hd, d)), (p + "attn.c_k.weight", "c_k", (kv, d)),
                (p + "attn.c_v.weight", "c_v", (kv, d)), (p + "attn.c_proj.weight", "attn.c_proj", (d, d))]
        if has_ve(i, L):
            out.append((p + "attn.ve_gate.weight", "ve_gate", (c["n_kv_head"], 12)))
        out += [(p + "mlp.c_fc.weight", "c_fc", (4 * d, d)), (p + "mlp.c_proj.weight", "mlp.c_proj", (d, 4 * d))]
    out += [("lm_head.weight", "lm_head", (v, d)), ("smear_gate.weight", "smear_gate", (1, 24))]
    out += [(f"value_embeds.{i}.weight", "value_embeds", (v, kv)) for i in range(L) if has_ve(i, L)]
    return out


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def bf16_bits(x):
    b = struct.unpack("<I", struct.pack("<f", x))[0]
    return (b + 0x7FFF + ((b >> 16) & 1)) >> 16


def bf16(x):
    return struct.unpack("<f", struct.pack("<I", bf16_bits(f32(x)) << 16))[0]


def stored_dtype(kind, rounding):
    if rounding is None:
        return "F64"
    return "BF16" if kind in ("wte", "value_embeds") else "F32"


def weights(c):
    params = {}
    for k, (name, kind, shape) in enumerate(inventory(c)):
        scale, offset = KINDS[kind]
        if scale is None:
            scale = c["head"]
        n = math.prod(shape)
        vals = [scale * math.sin(0.61 * e + 1.3 * k + 0.25) + offset for e in range(n)]
        dt = stored_dtype(kind, c["rounding"])
        if dt == "F32":
            vals = [f32(x) for x in vals]
        elif dt == "BF16":
            vals = [bf16(f32(x)) for x in vals]
        params[name] = (shape, vals)
    return params


def matrix(params, name):
    shape, vals = params[name]
    return [vals[r * shape[1]:(r + 1) * shape[1]] for r in range(shape[0])]


def mv(w, x):
    return [sum(a * b for a, b in zip(row, x)) for row in w]


def rms(x):
    r = math.sqrt(sum(v * v for v in x) / len(x) + EPS)
    return [v / r for v in x]


def sigmoid(z):
    return 1.0 / (1.0 + math.exp(-z))


def rotary(v, heads, hd, pos):
    """apply_rotary_emb: halves, y1 = x1 cos + x2 sin, y2 = -x1 sin + x2 cos."""
    out = list(v)
    half = hd // 2
    for h in range(heads):
        for i in range(half):
            ang = pos * (1.0 / (BASE ** ((2 * i) / hd)))
            co, si = math.cos(ang), math.sin(ang)
            x1, x2 = v[h * hd + i], v[h * hd + half + i]
            out[h * hd + i] = x1 * co + x2 * si
            out[h * hd + half + i] = x1 * (-si) + x2 * co
    return out


class Model:
    def __init__(self, c, params=None):
        self.c = c
        self.p = params or weights(c)
        self.d = c["n_embd"]
        self.hd = c["n_embd"] // c["n_head"]
        self.win = windows(c)

    def project(self, i, xn, token, pos):
        """q, k, v of one position in layer i (after rotary, QK norm and the 1.2 scale)."""
        c, p, hd = self.c, self.p, self.hd
        pre = f"transformer.h.{i}.attn."
        q = mv(matrix(p, pre + "c_q.weight"), xn)
        k = mv(matrix(p, pre + "c_k.weight"), xn)
        v = mv(matrix(p, pre + "c_v.weight"), xn)
        if has_ve(i, c["n_layer"]):
            ve = matrix(p, f"value_embeds.{i}.weight")[token]
            gate = [3 * sigmoid(g) for g in mv(matrix(p, pre + "ve_gate.weight"), xn[:12])]
            v = [v[j] + gate[j // hd] * ve[j] for j in range(len(v))]
        q, k = rotary(q, c["n_head"], hd, pos), rotary(k, c["n_kv_head"], hd, pos)
        q = [x * 1.2 for h in range(c["n_head"]) for x in rms(q[h * hd:(h + 1) * hd])]
        k = [x * 1.2 for h in range(c["n_kv_head"]) for x in rms(k[h * hd:(h + 1) * hd])]
        return q, k, v

    def attend(self, q, keys, values):
        c, hd = self.c, self.hd
        group = c["n_head"] // c["n_kv_head"]
        out = []
        for h in range(c["n_head"]):
            g = h // group
            s = [sum(q[h * hd + x] * kk[g * hd + x] for x in range(hd)) / math.sqrt(hd) for kk in keys]
            top = max(s)
            e = [math.exp(z - top) for z in s]
            tot = sum(e)
            out += [sum(e[j] / tot * values[j][g * hd + x] for j in range(len(keys))) for x in range(hd)]
        return out

    def finish(self, i, x, y):
        p = self.p
        pre = f"transformer.h.{i}."
        x = [a + b for a, b in zip(x, mv(matrix(p, pre + "attn.c_proj.weight"), y))]
        hdn = [max(z, 0.0) ** 2 for z in mv(matrix(p, pre + "mlp.c_fc.weight"), rms(x))]
        return [a + b for a, b in zip(x, mv(matrix(p, pre + "mlp.c_proj.weight"), hdn))]

    def embed(self, tokens, prev=None):
        """Normalized embeddings with smear; prev is the cached previous embedding."""
        p = self.p
        wte = matrix(p, "transformer.wte.weight")
        raw = [rms(wte[t]) for t in tokens]
        lam, sg = p["smear_lambda"][1][0], p["smear_gate.weight"][1]
        out = []
        for t in range(len(tokens)):
            before = raw[t - 1] if t > 0 else prev
            if before is None:
                out.append(raw[t])
            else:
                gate = lam * sigmoid(sum(a * b for a, b in zip(sg, raw[t][:24])))
                out.append([a + gate * b for a, b in zip(raw[t], before)])
        return out, raw

    def head(self, x, xb):
        p, c = self.p, self.c
        if xb is not None:
            lam = p["backout_lambda"][1][0]
            x = [a - lam * b for a, b in zip(x, xb)]
        z = mv(matrix(p, "lm_head.weight"), rms(x))[:c["vocab_size"]]
        return [SOFTCAP * math.tanh(v / SOFTCAP) for v in z]

    def full(self, tokens):
        """Training path: all positions, mask col<=row and row-col<=window."""
        c, p = self.c, self.p
        assert len(tokens) > 1
        x, _ = self.embed(tokens)
        x0 = [list(r) for r in x]
        resid, x0l = p["resid_lambdas"][1], p["x0_lambdas"][1]
        blocks, xb = [], None
        for i in range(c["n_layer"]):
            x = [[resid[i] * a + x0l[i] * b for a, b in zip(r, r0)] for r, r0 in zip(x, x0)]
            qkv = [self.project(i, rms(x[t]), tokens[t], t) for t in range(len(tokens))]
            ys = []
            for t in range(len(tokens)):
                vis = [j for j in range(len(tokens)) if j <= t and t - j <= self.win[i]]
                ys.append(self.attend(qkv[t][0], [qkv[j][1] for j in vis], [qkv[j][2] for j in vis]))
            x = [self.finish(i, x[t], ys[t]) for t in range(len(tokens))]
            blocks.append([list(r) for r in x])
            if i == c["n_layer"] // 2:
                xb = [list(r) for r in x]
        return [self.head(x[t], xb[t]) for t in range(len(tokens))], blocks

    def cached(self, tokens):
        """Inference path: one token at a time; flash_attn_with_kvcache's SDPA fallback
        appends k/v, then keeps the last window+1 keys when the window is shorter."""
        c, p = self.c, self.p
        kc = [[] for _ in range(c["n_layer"])]
        vc = [[] for _ in range(c["n_layer"])]
        prev, logits = None, []
        resid, x0l = p["resid_lambdas"][1], p["x0_lambdas"][1]
        for pos, tok in enumerate(tokens):
            xs, raw = self.embed([tok], prev)
            prev = raw[0]
            x = xs[0]
            x0, xb = list(x), None
            for i in range(c["n_layer"]):
                x = [resid[i] * a + x0l[i] * b for a, b in zip(x, x0)]
                q, k, v = self.project(i, rms(x), tok, pos)
                kc[i].append(k)
                vc[i].append(v)
                keys, values, w = kc[i], vc[i], self.win[i]
                if 0 <= w < len(keys):
                    start = max(0, len(keys) - (w + 1))
                    keys, values = keys[start:], values[start:]
                x = self.finish(i, x, self.attend(q, keys, values))
                if i == c["n_layer"] // 2:
                    xb = list(x)
            logits.append(self.head(x, xb))
        return logits


def loss(z, tokens):
    total = 0.0
    for t in range(len(tokens) - 1):
        top = max(z[t])
        total += top + math.log(sum(math.exp(v - top) for v in z[t])) - z[t][tokens[t + 1]]
    return total / (len(tokens) - 1)


def run(name):
    c = CASES[name]
    m = Model(c)
    z, blocks = m.full(c["tokens"])
    zc = m.cached(c["tokens"])
    gap = max(abs(a - b) for r, rc in zip(z, zc) for a, b in zip(r, rc))
    assert gap < 1e-12, (name, gap)
    return m, z, blocks, loss(z, c["tokens"]), gap


def check():
    for name in CASES:
        m, z, blocks, l, gap = run(name)
        print(f"{name}: {len(m.p)} tensors, {sum(len(v) for _, v in m.p.values())} values, windows {m.win}, "
              f"loss {l!r}, cached-vs-full {gap:.3g}")
    # The window bites exactly where the mask says it should.
    c = dict(CASES["window"])
    z, _ = Model(c).full(c["tokens"])
    c["window_pattern"] = "L"
    zl, _ = Model(c).full(c["tokens"])
    assert all(z[t] == zl[t] for t in range(129)) and z[129] != zl[129]
    # A checkpoint's rounding is exact: f32 and bf16 values round-trip.
    assert bf16(1.0 + 2 ** -8) == 1.0 and bf16(1.0 + 3 * 2 ** -8) == 1.0 + 2 ** -6 and bf16(-0.0) == 0.0
    print("nanochat oracle: full and cached paths agree, the window bites at position 129 only")


def coax_list(vals):
    return ",".join(repr(float(v)) for v in vals)


def emit_coax():
    m, z, blocks, l, _ = run("tiny")
    print(f"NC_ORACLE_TINY_LOSS :: {l!r};")
    flat = [v for r in z for v in r]
    print(f"NC_ORACLE_TINY_LOGITS :: [{len(flat)}]f64.[{coax_list(flat)}];")
    print(f"NC_ORACLE_TINY_BLOCK :: [{len(blocks[0]) * 24}]f64.[{coax_list([v for r in blocks[0] for v in r])}];")
    m, z, blocks, l, _ = run("window")
    print(f"NC_ORACLE_WINDOW_LOSS :: {l!r};")
    flat = [v for t in WINDOW_LOGIT_POSITIONS for v in z[t]]
    print(f"NC_ORACLE_WINDOW_LOGITS :: [{len(flat)}]f64.[{coax_list(flat)}];")
    flat = [v for b in blocks for t in WINDOW_BLOCK_POSITIONS for v in b[t]]
    print(f"NC_ORACLE_WINDOW_BLOCKS :: [{len(flat)}]f64.[{coax_list(flat)}];")


def emit_header():
    print("/* Generated by psionic/tests/nanochat_oracle.py --header. */")
    for name in ("tiny", "window", "checkpoint"):
        m, z, blocks, l, _ = run(name)
        up = name.upper()
        toks = CASES[name]["tokens"]
        print(f"static const int NC_{up}_TOKENS[{len(toks)}] = {{{','.join(map(str, toks))}}};")
        print(f"static const double NC_{up}_LOSS = {l!r};")
        flat = [v for r in z for v in r]
        print(f"static const double NC_{up}_LOGITS[{len(flat)}] = {{{','.join(repr(v) for v in flat)}}};")
        flat = [v for b in blocks for r in b for v in r]
        print(f"static const double NC_{up}_BLOCKS[{len(flat)}] = {{{','.join(repr(v) for v in flat)}}};")


def emit_weights(name):
    c = CASES[name]
    m = Model(c)
    out = {"source": SOURCE, "case": name,
           "model_config": {k: c[k] for k in ("sequence_len", "vocab_size", "n_layer", "n_head", "n_kv_head", "n_embd", "window_pattern")},
           "tensors": [{"name": n, "dtype": stored_dtype(kind, c["rounding"]), "shape": list(shape), "values": m.p[n][1]}
                       for n, kind, shape in inventory(c)]}
    for t in out["tensors"]:
        if t["dtype"] == "BF16":
            t["bits"] = [bf16_bits(v) for v in t["values"]]
    print(json.dumps(out))


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--coax", action="store_true")
    a.add_argument("--header", action="store_true")
    a.add_argument("--weights", choices=sorted(CASES))
    o = a.parse_args()
    if o.coax:
        emit_coax()
    elif o.header:
        emit_header()
    elif o.weights:
        emit_weights(o.weights)
    else:
        check()


if __name__ == "__main__":
    main()
