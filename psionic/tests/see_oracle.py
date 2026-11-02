"""Independent binary64 oracle for chapter 143's vision encoder.

Follows the Qwen3-VL image processor and vision model as written upstream
(reshape and transpose of the processor's patch layout, linspace position
interpolation, the rotary table and rotate_half, LayerNorm, the 2 by 2 merger)
rather than psionic/lib/see.coax's loops. Python is tooling only.

    python3 -I psionic/tests/see_oracle.py tiny > psionic/fixtures/see/oracle.coax
The same functions serve psionic/tests/vision_slices.py on original weights.
"""
import math
import sys


def linspace(a, b, n):
    if n == 1:
        return [float(a)]
    step = (b - a) / (n - 1)
    return [a + i * step for i in range(n)]


def processor_patches(pixels, height, width, patch, temporal, merge):
    """pixels: height*width*3 bytes (RGB). Returns rows in the processor's order."""
    chw = [[[(pixels[3 * (y * width + x) + c] / 255 - 0.5) / 0.5 for x in range(width)] for y in range(height)] for c in range(3)]
    frames = [chw] * temporal  # a still image fills every frame of the temporal patch
    gh, gw = height // patch, width // patch
    rows = []
    # reshape (t, T, C, gh/m, m, P, gw/m, m, P), transpose (0, 3, 6, 4, 7, 2, 1, 5, 8)
    for bh in range(gh // merge):
        for bw in range(gw // merge):
            for mh in range(merge):
                for mw in range(merge):
                    row = []
                    for c in range(3):
                        for t in range(temporal):
                            for py in range(patch):
                                for px in range(patch):
                                    y = (bh * merge + mh) * patch + py
                                    x = (bw * merge + mw) * patch + px
                                    row.append(frames[t][c][y][x])
                    rows.append(row)
    return rows, gh, gw


def linear(x, w, b, outputs):
    inputs = len(x[0])
    return [[b[o] + sum(xi * wi for xi, wi in zip(r, w[o * inputs:(o + 1) * inputs])) for o in range(outputs)] for r in x]


def layernorm(x, g, b, eps):
    out = []
    for r in x:
        m = sum(r) / len(r)
        v = sum((a - m) ** 2 for a in r) / len(r)
        out.append([(a - m) / math.sqrt(v + eps) * g[i] + b[i] for i, a in enumerate(r)])
    return out


def merged_order(gh, gw, merge):
    """(row, col) of each patch: view(h/m, m, w/m, m).permute(0, 2, 1, 3)."""
    return [(bh * merge + mh, bw * merge + mw) for bh in range(gh // merge) for bw in range(gw // merge)
            for mh in range(merge) for mw in range(merge)]


def pos_embed(table, side, width, gh, gw, merge):
    hs, ws = linspace(0, side - 1, gh), linspace(0, side - 1, gw)
    raster = []
    for h in hs:
        for w in ws:
            hf, wf = int(h), int(w)
            hc, wc = min(hf + 1, side - 1), min(wf + 1, side - 1)
            dh, dw = h - hf, w - wf
            terms = [(hf, wf, (1 - dh) * (1 - dw)), (hf, wc, (1 - dh) * dw), (hc, wf, dh * (1 - dw)), (hc, wc, dh * dw)]
            raster.append([sum(wt * table[(r * side + c) * width + k] for r, c, wt in terms) for k in range(width)])
    return [raster[r * gw + c] for r, c in merged_order(gh, gw, merge)]


def rotary(gh, gw, merge, head_dim, theta):
    dim = head_dim // 2
    inv = [1.0 / theta ** (i / dim) for i in range(0, dim, 2)]
    out = []
    for r, c in merged_order(gh, gw, merge):
        emb = [r * f for f in inv] + [c * f for f in inv]
        emb = emb + emb
        out.append(([math.cos(a) for a in emb], [math.sin(a) for a in emb]))
    return out


def gelu_tanh(z):
    return 0.5 * z * (1 + math.tanh(math.sqrt(2 / math.pi) * (z + 0.044715 * z ** 3)))


def gelu(z):
    return 0.5 * z * (1 + math.erf(z / math.sqrt(2)))


def block(x, p, gh, gw, merge, heads, theta, eps):
    n, width = len(x), len(x[0])
    hd = width // heads
    h = layernorm(x, p["norm1"], p["norm1_bias"], eps)
    qkv = linear(h, p["qkv"], p["qkv_bias"], 3 * width)
    rot = rotary(gh, gw, merge, hd, theta)

    def rope(v, cos, sin):
        half = len(v) // 2
        rh = [-a for a in v[half:]] + v[:half]
        return [v[i] * cos[i] + rh[i] * sin[i] for i in range(len(v))]

    att = [[0.0] * width for _ in range(n)]
    for head in range(heads):
        q = [rope(qkv[r][head * hd:(head + 1) * hd], *rot[r]) for r in range(n)]
        k = [rope(qkv[r][width + head * hd:width + (head + 1) * hd], *rot[r]) for r in range(n)]
        v = [qkv[r][2 * width + head * hd:2 * width + (head + 1) * hd] for r in range(n)]
        for i in range(n):
            s = [sum(a * b for a, b in zip(q[i], k[j])) * hd ** -0.5 for j in range(n)]
            m = max(s)
            e = [math.exp(a - m) for a in s]
            t = sum(e)
            for d in range(hd):
                att[i][head * hd + d] = sum(e[j] * v[j][d] for j in range(n)) / t
    mid = [[a + b for a, b in zip(r1, r2)] for r1, r2 in zip(linear(att, p["proj"], p["proj_bias"], width), x)]
    h = layernorm(mid, p["norm2"], p["norm2_bias"], eps)
    f = [[gelu_tanh(a) for a in r] for r in linear(h, p["fc1"], p["fc1_bias"], len(p["fc1_bias"]))]
    return [[a + b for a, b in zip(r1, r2)] for r1, r2 in zip(linear(f, p["fc2"], p["fc2_bias"], width), mid)]


def merger(x, p, merge, eps):
    h = layernorm(x, p["norm"], p["norm_bias"], eps)
    per = merge * merge
    rows = [sum(h[i * per:(i + 1) * per], []) for i in range(len(h) // per)]
    z = [[gelu(a) for a in r] for r in linear(rows, p["fc1"], p["fc1_bias"], len(p["fc1_bias"]))]
    return linear(z, p["fc2"], p["fc2_bias"], len(p["fc2_bias"]))


def fill(n, seed):
    return [((i * seed + 11) % 17 - 8) / 16 for i in range(n)]


def split(values, names_sizes):
    out, at = {}, 0
    for name, size in names_sizes:
        out[name] = values[at:at + size]
        at += size
    return out


def case(patch, temporal, merge, width, heads, inner, side, outw, height, wide, block_size, merger_size):
    pixels = [(i * 37 + 11) % 256 for i in range(height * wide * 3)]
    rows, gh, gw = processor_patches(pixels, height, wide, patch, temporal, merge)
    features = 3 * temporal * patch * patch
    pw, pb, table = fill(width * features, 5), fill(width, 3), fill(side * side * width, 7)
    emb = [[a + b for a, b in zip(r, pos)] for r, pos in
           zip(linear(rows, pw, pb, width), pos_embed(table, side, width, gh, gw, merge))]
    bp = split(fill(block_size, 11), [("norm1", width), ("norm1_bias", width), ("qkv", 3 * width * width), ("qkv_bias", 3 * width),
                                      ("proj", width * width), ("proj_bias", width), ("norm2", width), ("norm2_bias", width),
                                      ("fc1", inner * width), ("fc1_bias", inner), ("fc2", width * inner), ("fc2_bias", width)])
    blocked = block(emb, bp, gh, gw, merge, heads, 10000.0, 1e-6)
    k = merge * merge * width
    mp = split(fill(merger_size, 13), [("norm", width), ("norm_bias", width), ("fc1", k * k), ("fc1_bias", k), ("fc2", outw * k), ("fc2_bias", outw)])
    return rows, emb, blocked, merger(blocked, mp, merge, 1e-6)


def tiny():
    def arr(name, vals):
        flat = [v for r in vals for v in r]
        body = ",".join(repr(v) for v in flat)
        return f"{name} :: [{len(flat)}]f64.[{body}];\n"

    sys.stdout.write("// Generated by psionic/tests/see_oracle.py tiny: the independent binary64\n"
                     "// oracle for chapter 143's tiny encoders. Do not edit.\n"
                     "// A stored f32 value against the oracle: within 1e-6 of it, relative to\n"
                     "// 1 + its size; the merger's erf adds up to 1.5e-7 per value.\n"
                     "sv_near :: (a:[]f32,b:[]f64)->bool {\n"
                     "    if a.count!=b.count {return false;}\n"
                     "    for i:0..<a.count {if !(abs(f64(a[i])-b[i])<=0.000001*(1+abs(b[i]))) {return false;}}\n"
                     "    return true;\n}\n")
    a = case(2, 2, 2, 8, 2, 6, 3, 5, 4, 8, 430, 1237)
    b = case(3, 1, 3, 8, 1, 5, 2, 3, 9, 18, 413, 5491)
    for prefix, vals in (("SV_ORACLE", a), ("SV_ORACLE_B", b)):
        for name, v in zip(("PATCHES", "EMBED", "BLOCK", "MERGE"), vals):
            sys.stdout.write(arr(f"{prefix}_{name}", v))


if __name__ == "__main__":
    if sys.argv[1:] == ["tiny"]:
        tiny()
    else:
        sys.exit("usage: see_oracle.py tiny")
