"""Independent binary64 oracle for chapter 144's C-RADIO encoder.

Written from the shapes and operations of the pinned C-RADIOv3-B reference
(read as text only, never run): the input conditioner (bytes / 255, then the
CLIP mean and spread), Im2Patches' rearrange 'c (py yy) (px xx) -> (py px)
(c yy xx)', the bias-free patch linear, the CPE position table resized with
F.interpolate(bilinear, align_corners=True) to a square of the longer grid
side and cut to the grid's top-left corner, the cls and register tokens
prepended, timm's pre-norm Block with layer scale, the summary tokens chosen
by summary_idxs and the spatial features after the prefix. It does not
follow psionic/lib/radio.coax's loops. Python is tooling only.

    python3 -I psionic/tests/radio_oracle.py tiny > psionic/fixtures/radio/oracle.coax
The same functions serve psionic/tests/radio_slices.py on original weights.
"""
import math
import operator
import sys

CLIP_MEAN = (0.48145467042922974, 0.45782750844955444, 0.40821072459220886)
CLIP_STD = (0.2686295509338379, 0.2613025903701782, 0.27577710151672363)


def conditioned(pixels, height, width, mean, std):
    """pixels: height*width*3 bytes (RGB). Returns [c][y][x]."""
    return [[[(pixels[3 * (y * width + x) + c] / 255 - mean[c]) / std[c] for x in range(width)] for y in range(height)] for c in range(3)]


def im2patches(chw, patch):
    height, width = len(chw[0]), len(chw[0][0])
    rows = []
    for py in range(height // patch):
        for px in range(width // patch):
            rows.append([chw[c][py * patch + yy][px * patch + xx] for c in range(3) for yy in range(patch) for xx in range(patch)])
    return rows


def linear(x, w, b, outputs):
    """y = x W^T + b, each sum exact in binary64 (math.fsum) and rounded once."""
    inputs = len(x[0])
    rows = [w[o * inputs:(o + 1) * inputs] for o in range(outputs)]
    bias = b if b is not None else [0.0] * outputs
    return [[bias[o] + math.fsum(map(operator.mul, r, rows[o])) for o in range(outputs)] for r in x]


def resize_axis(n_in, n_out):
    """torch's align_corners source index per output index: (low, high, lambda)."""
    out = []
    for i in range(n_out):
        src = i * (n_in - 1) / (n_out - 1) if n_out > 1 else 0.0
        lo = int(math.floor(src))
        hi = min(lo + 1, n_in - 1)
        out.append((lo, hi, src - lo))
    return out


def cpe_positions(table, side, width, rows, cols):
    """table: side*side rows of width. Interpolate (side, side) -> (m, m), m =
    max(rows, cols), then keep [:rows, :cols]; flatten row-major."""
    m = max(rows, cols)
    if (rows, cols) == (side, side):
        return [table[i * width:(i + 1) * width] for i in range(side * side)]
    ay, ax = resize_axis(side, m), resize_axis(side, m)
    out = []
    for y in range(rows):
        y0, y1, ly = ay[y]
        for x in range(cols):
            x0, x1, lx = ax[x]
            def at(r, c, k):
                return table[(r * side + c) * width + k]
            out.append([(1 - ly) * ((1 - lx) * at(y0, x0, k) + lx * at(y0, x1, k)) + ly * ((1 - lx) * at(y1, x0, k) + lx * at(y1, x1, k))
                        for k in range(width)])
    return out


def layernorm(x, g, b, eps):
    out = []
    for r in x:
        m = math.fsum(r) / len(r)
        v = math.fsum((a - m) ** 2 for a in r) / len(r)
        s = 1 / math.sqrt(v + eps)
        out.append([(a - m) * s * g[i] + b[i] for i, a in enumerate(r)])
    return out


def attention(qkv, heads, width):
    hd = width // heads
    out = [[0.0] * width for _ in qkv]
    for h in range(heads):
        q = [r[h * hd:(h + 1) * hd] for r in qkv]
        k = [r[width + h * hd:width + (h + 1) * hd] for r in qkv]
        v = [r[2 * width + h * hd:2 * width + (h + 1) * hd] for r in qkv]
        for i in range(len(qkv)):
            s = [math.fsum(a * b for a, b in zip(q[i], kj)) / math.sqrt(hd) for kj in k]
            top = max(s)
            e = [math.exp(a - top) for a in s]
            z = math.fsum(e)
            for d in range(hd):
                out[i][h * hd + d] = math.fsum(e[j] * v[j][d] for j in range(len(v))) / z
    return out


def gelu(z):
    return 0.5 * z * (1 + math.erf(z / math.sqrt(2)))


def block(x, p, heads, inner, eps):
    width = len(x[0])
    h = layernorm(x, p["norm1"], p["norm1_bias"], eps)
    a = linear(attention(linear(h, p["qkv"], p["qkv_bias"], 3 * width), heads, width), p["proj"], p["proj_bias"], width)
    x = [[xi + ai * g for xi, ai, g in zip(r, s, p["ls1"])] for r, s in zip(x, a)]
    h = layernorm(x, p["norm2"], p["norm2_bias"], eps)
    f = [[gelu(v) for v in r] for r in linear(h, p["fc1"], p["fc1_bias"], inner)]
    m = linear(f, p["fc2"], p["fc2_bias"], width)
    return [[xi + mi * g for xi, mi, g in zip(r, s, p["ls2"])] for r, s in zip(x, m)]


def embed(patches, w, table, side, width, rows, cols, tokens):
    pos = cpe_positions(table, side, width, rows, cols)
    y = [[a + b for a, b in zip(r, q)] for r, q in zip(linear(patches, w, None, width), pos)]
    prefix = [tokens[i * width:(i + 1) * width] for i in range(len(tokens) // width)]
    return prefix + y


def outputs(y, cls, summary_idxs, skip):
    head = y[:cls]
    return [v for i in summary_idxs for v in head[i]], y[skip:]


BLOCK_PARTS = ("norm1", "norm1_bias", "qkv", "qkv_bias", "proj", "proj_bias", "ls1", "norm2", "norm2_bias", "fc1", "fc1_bias",
               "fc2", "fc2_bias", "ls2")


def block_sizes(width, inner):
    return (width, width, 3 * width * width, 3 * width, width * width, width, width, width, width, inner * width, inner,
            width * inner, width, width)


def fill(n, seed):
    return [((i * seed + 11) % 17 - 8) / 16 for i in range(n)]


def tiny_case(height, width_px):
    patch, width, heads, inner, depth, side, cls, registers, summary = 2, 8, 2, 6, 2, 4, 3, 1, (0, 1)
    pixels = [(i * 37 + 11) % 256 for i in range(3 * height * width_px)]
    patches = im2patches(conditioned(pixels, height, width_px, CLIP_MEAN, CLIP_STD), patch)
    rows, cols = height // patch, width_px // patch
    w, table, tokens = fill(width * 3 * patch * patch, 5), fill(side * side * width, 7), fill((cls + registers) * width, 3)
    y = embed(patches, w, table, side, width, rows, cols, tokens)
    emb = y
    flat = fill(depth * sum(block_sizes(width, inner)), 11)
    at = 0
    first = None
    for _ in range(depth):
        p = {}
        for name, n in zip(BLOCK_PARTS, block_sizes(width, inner)):
            p[name] = flat[at:at + n]
            at += n
        y = block(y, p, heads, inner, 1e-6)
        if first is None:
            first = y
    s, f = outputs(y, cls, summary, cls + registers)
    return patches, emb, first, [s], f


def tiny():
    def arr(name, vals):
        flat = [v for r in vals for v in r]
        return f"{name} :: [{len(flat)}]f64.[{','.join(repr(v) for v in flat)}];\n"

    sys.stdout.write("// Generated by psionic/tests/radio_oracle.py tiny: the independent binary64\n"
                     "// oracle for chapter 144's tiny C-RADIO encoders. Do not edit.\n"
                     "// A stored f32 value against the oracle: within 1e-6 of it, relative to\n"
                     "// 1 + its size; the MLP's erf adds up to 1.5e-7 per value.\n"
                     "rd_near :: (a:[]f32,b:[]f64)->bool {\n"
                     "    if a.count!=b.count {return false;}\n"
                     "    for i:0..<a.count {if !(abs(f64(a[i])-b[i])<=0.000001*(1+abs(b[i]))) {return false;}}\n"
                     "    return true;\n}\n")
    for prefix, (h, w) in (("RD_ORACLE_WIDE", (4, 6)), ("RD_ORACLE_TALL", (6, 4))):
        for name, v in zip(("PATCHES", "EMBED", "BLOCK", "SUMMARY", "FEATURES"), tiny_case(h, w)):
            sys.stdout.write(arr(f"{prefix}_{name}", v))


if __name__ == "__main__":
    if sys.argv[1:] == ["tiny"]:
        tiny()
    else:
        sys.exit("usage: radio_oracle.py tiny")
