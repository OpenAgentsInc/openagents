"""Chapter 153's original-weight slices of nvidia/parakeet-tdt-0.6b-v3: an
independent binary64 oracle in plain Python, written from NeMo's published
preprocessor and FastConformer encoder, and the comparison with the Coax run.

Firm reason: 2 (an independent oracle).

    python3 -I psionic/tests/parakeet_oracle.py ranges MODEL_DIRECTORY OUT_DIR
    python3 -I psionic/tests/parakeet_oracle.py compare MODEL_DIRECTORY OUT_DIR CLIP.pcm

ranges reads the safetensors header (JSON only) and writes each tensor's
absolute byte range. compare recomputes, for one clip, the log-mel features
from the PCM samples, the subsampling from the Coax features, encoder layer 0
from the Coax subsampled frames and layer 1 from the Coax layer 0, and checks
each within the frozen tolerance. Each stage starts from the Coax stage
before it, so a difference points at one stage. Nothing from the download is
imported or run. Tooling only.
"""
import array
import json
import math
import operator
import pathlib
import struct
import sys

# Frozen on 2026-11-02 before the first comparison: every value of a slice
# may differ from this oracle by at most TOLERANCE * (1 + the largest
# |oracle value| of that slice).
TOLERANCE = 1e-4
RATE, N_FFT, WIN, HOP, MELS, PREEMPH = 16000, 512, 400, 160, 128, 0.97
GUARD = 2.0 ** -24
D, HEADS, HIDDEN, KERNEL, CH = 1024, 8, 4096, 9, 256
EPS = 1e-5


def header(directory):
    path = pathlib.Path(directory) / "model.safetensors"
    with path.open("rb") as f:
        n = struct.unpack("<Q", f.read(8))[0]
        if not 2 <= n <= 1 << 20:
            raise SystemExit(f"refused: header of {n} bytes")
        return path, 8 + n, json.loads(f.read(n))


class Weights:
    def __init__(self, directory):
        self.path, self.base, self.header = header(directory)
        self.f = self.path.open("rb")

    def get(self, name):
        t = self.header[name]
        if t["dtype"] != "F32":
            raise SystemExit(f"{name}: {t['dtype']}")
        b, e = t["data_offsets"]
        self.f.seek(self.base + b)
        a = array.array("f")
        a.frombytes(self.f.read(e - b))
        return a

    def rows(self, name, cols):
        w = self.get(name)
        return [w[r * cols:(r + 1) * cols] for r in range(len(w) // cols)]


# The preprocessor: preemphasis, a centred 512-point frame with zeros
# outside the signal, a symmetric 400-point Hann window in its middle, the
# power spectrum by a direct DFT, Slaney mel filters as librosa defines them,
# log(x + 2^-24), then each mel standardized over frames (n - 1 deviation,
# plus 1e-5).
def slaney_mel(f):
    return 15 + math.log(f / 1000) / (math.log(6.4) / 27) if f >= 1000 else f / (200 / 3)


def slaney_hz(m):
    return 1000 * math.exp((math.log(6.4) / 27) * (m - 15)) if m >= 15 else (200 / 3) * m


def mel_bank():
    lo, hi = slaney_mel(0.0), slaney_mel(RATE / 2)
    edges = [slaney_hz(lo + (hi - lo) * i / (MELS + 1)) for i in range(MELS + 2)]
    freqs = [k * RATE / N_FFT for k in range(N_FFT // 2 + 1)]
    bank = []
    for i in range(MELS):
        row = []
        for f in freqs:
            lower = (f - edges[i]) / (edges[i + 1] - edges[i])
            upper = (edges[i + 2] - f) / (edges[i + 2] - edges[i + 1])
            row.append(max(0.0, min(lower, upper)) * 2 / (edges[i + 2] - edges[i]))
        bank.append(row)
    return bank


def features(pcm):
    x = [v / 32768 for v in pcm]
    y = [x[0]] + [x[i] - PREEMPH * x[i - 1] for i in range(1, len(x))]
    padded = [0.0] * (N_FFT // 2) + y + [0.0] * (N_FFT // 2)
    frames = len(x) // HOP + 1
    left = (N_FFT - WIN) // 2
    window = [0.5 - 0.5 * math.cos(2 * math.pi * n / (WIN - 1)) for n in range(WIN)]
    cos = [math.cos(2 * math.pi * k / N_FFT) for k in range(N_FFT)]
    sin = [math.sin(2 * math.pi * k / N_FFT) for k in range(N_FFT)]
    bank = mel_bank()
    out = [[0.0] * frames for _ in range(MELS)]
    for t in range(frames):
        seg = [padded[t * HOP + left + n] * window[n] for n in range(WIN)]
        power = []
        for k in range(N_FFT // 2 + 1):
            re = im = 0.0
            for n in range(WIN):
                idx = (k * (n + left)) % N_FFT
                re += seg[n] * cos[idx]
                im -= seg[n] * sin[idx]
            power.append(re * re + im * im)
        for m in range(MELS):
            out[m][t] = math.log(sum(map(operator.mul, bank[m], power)) + GUARD)
    for m in range(MELS):
        row = out[m]
        mean = sum(row) / frames
        dev = math.sqrt(sum((v - mean) ** 2 for v in row) / (frames - 1)) + EPS
        out[m] = [(v - mean) / dev for v in row]
    return out, frames


# The subsampling: Conv2d(1, 256, 3, stride 2, pad 1), ReLU, then twice a
# depthwise Conv2d(256, 256, 3, stride 2, pad 1, groups 256), a pointwise
# Conv2d(256, 256, 1) and ReLU; then (channel, frequency) flattened channel
# first and a linear map to 1,024. Time is the image's height.
def conv3(img, h, w, kernel, bias, depthwise):
    oh, ow = (h - 1) // 2 + 1, (w - 1) // 2 + 1
    out = []
    for c in range(CH):
        src = img[c] if depthwise else img[0]
        k = kernel[c * 9:(c + 1) * 9]
        plane = []
        for t in range(oh):
            for f in range(ow):
                s = bias[c]
                for a in range(3):
                    r = 2 * t - 1 + a
                    if 0 <= r < h:
                        for b in range(3):
                            q = 2 * f - 1 + b
                            if 0 <= q < w:
                                s += k[a * 3 + b] * src[r * w + q]
                plane.append(s)
        out.append(plane)
    return out, oh, ow


def pointwise(img, rows, bias):
    n = len(img[0])
    cols = [[img[c][i] for c in range(CH)] for i in range(n)]
    return [[bias[o] + sum(map(operator.mul, rows[o], cols[i])) for i in range(n)] for o in range(CH)]


def relu(img):
    return [[v if v > 0 else 0.0 for v in plane] for plane in img]


def subsample(wt, feats, frames):
    p = "encoder.subsampling."
    img = [[feats[m][t] for t in range(frames) for m in range(MELS)]]
    a, h, w = conv3(img, frames, MELS, wt.get(p + "layers.0.weight"), wt.get(p + "layers.0.bias"), False)
    a = relu(a)
    a, h, w = conv3(a, h, w, wt.get(p + "layers.2.weight"), wt.get(p + "layers.2.bias"), True)
    a = relu(pointwise(a, wt.rows(p + "layers.3.weight", CH), wt.get(p + "layers.3.bias")))
    a, h, w = conv3(a, h, w, wt.get(p + "layers.5.weight"), wt.get(p + "layers.5.bias"), True)
    a = relu(pointwise(a, wt.rows(p + "layers.6.weight", CH), wt.get(p + "layers.6.bias")))
    rows, bias = wt.rows(p + "linear.weight", CH * w), wt.get(p + "linear.bias")
    out = []
    for t in range(h):
        v = [a[c][t * w + f] for c in range(CH) for f in range(w)]
        out.append([bias[o] + sum(map(operator.mul, rows[o], v)) for o in range(D)])
    return out


def norm(x, g, b):
    mean = sum(x) / len(x)
    var = sum((v - mean) ** 2 for v in x) / len(x)
    r = 1 / math.sqrt(var + EPS)
    return [(v - mean) * r * g[i] + b[i] for i, v in enumerate(x)]


def matvec(rows, x):
    return [sum(map(operator.mul, r, x)) for r in rows]


def silu(v):
    return v / (1 + math.exp(-v)) if v > -700 else 0.0


def sigmoid(v):
    return 1 / (1 + math.exp(-v)) if v > -700 else 0.0


def feed_forward(wt, p, k, h):
    g, b = wt.get(p + f"norm_feed_forward{k}.weight"), wt.get(p + f"norm_feed_forward{k}.bias")
    w1, w2 = wt.rows(p + f"feed_forward{k}.linear1.weight", D), wt.rows(p + f"feed_forward{k}.linear2.weight", HIDDEN)
    return [[a + 0.5 * y for a, y in zip(x, matvec(w2, [silu(v) for v in matvec(w1, norm(x, g, b))]))] for x in h]


# Self-attention with Transformer-XL relative positions, as NeMo's
# RelPositionMultiHeadAttention: score(i, j) = ((q_i + u) . k_j + (q_i + v) .
# P(i - j)) / sqrt(128), where P(r) is linear_pos (no bias) of the sinusoid
# of relative position r. No biases on q, k, v or the output map.
def attention(wt, p, h):
    L, dk = len(h), D // HEADS
    g, b = wt.get(p + "norm_self_att.weight"), wt.get(p + "norm_self_att.bias")
    a = p + "self_attn."
    wq, wk, wv, wo = (wt.rows(a + f"{n}_proj.weight", D) for n in "qkvo")
    wp = wt.rows(a + "relative_k_proj.weight", D)
    u, bv = wt.get(a + "bias_u"), wt.get(a + "bias_v")
    xs = [norm(x, g, b) for x in h]
    q, k, v = [matvec(wq, x) for x in xs], [matvec(wk, x) for x in xs], [matvec(wv, x) for x in xs]
    pos = {}
    for r in range(-(L - 1), L):
        s = []
        for m in range(D // 2):
            f = math.exp(2 * m * -(math.log(10000.0) / D))
            s += [math.sin(r * f), math.cos(r * f)]
        pos[r] = matvec(wp, s)
    ctx = [[0.0] * D for _ in range(L)]
    for hd in range(HEADS):
        lo, hi = hd * dk, (hd + 1) * dk
        for i in range(L):
            qu = [q[i][m] + u[m] for m in range(lo, hi)]
            qv = [q[i][m] + bv[m] for m in range(lo, hi)]
            scores = [(sum(map(operator.mul, qu, k[j][lo:hi])) + sum(map(operator.mul, qv, pos[i - j][lo:hi]))) / math.sqrt(dk) for j in range(L)]
            top = max(scores)
            e = [math.exp(s - top) for s in scores]
            z = sum(e)
            for m in range(lo, hi):
                ctx[i][m] = sum(e[j] * v[j][m] for j in range(L)) / z
    return [[a + y for a, y in zip(x, matvec(wo, c))] for x, c in zip(h, ctx)]


# The convolution module: norm, pointwise to 2,048, GLU, depthwise over time
# (9 taps, 4 zero frames each side), batch norm with stored statistics, swish,
# pointwise back. No biases on the three convolutions.
def convolution(wt, p, h):
    L = len(h)
    c = p + "conv."
    g, b = wt.get(p + "norm_conv.weight"), wt.get(p + "norm_conv.bias")
    pw1, pw2 = wt.rows(c + "pointwise_conv1.weight", D), wt.rows(c + "pointwise_conv2.weight", D)
    dw = wt.get(c + "depthwise_conv.weight")
    mean, var = wt.get(c + "norm.running_mean"), wt.get(c + "norm.running_var")
    gain, shift = wt.get(c + "norm.weight"), wt.get(c + "norm.bias")
    gated = []
    for x in h:
        two = matvec(pw1, norm(x, g, b))
        gated.append([two[i] * sigmoid(two[D + i]) for i in range(D)])
    out = []
    for t in range(L):
        y = []
        for ch in range(D):
            s = 0.0
            for j in range(KERNEL):
                u = t + j - KERNEL // 2
                if 0 <= u < L:
                    s += dw[ch * KERNEL + j] * gated[u][ch]
            y.append(silu((s - mean[ch]) / math.sqrt(var[ch] + EPS) * gain[ch] + shift[ch]))
        out.append([a + z for a, z in zip(h[t], matvec(pw2, y))])
    return out


def layer(wt, n, h):
    p = f"encoder.layers.{n}."
    h = feed_forward(wt, p, 1, h)
    h = attention(wt, p, h)
    h = convolution(wt, p, h)
    h = feed_forward(wt, p, 2, h)
    g, b = wt.get(p + "norm_out.weight"), wt.get(p + "norm_out.bias")
    return [norm(x, g, b) for x in h]


def load(out, name, count):
    a = array.array("f")
    with (pathlib.Path(out) / name).open("rb") as f:
        a.frombytes(f.read())
    if len(a) != count:
        raise SystemExit(f"{name}: {len(a)} values, expected {count}")
    return a


def gap(name, got, ref):
    flat = [v for r in ref for v in r]
    worst = max(abs(a - b) for a, b in zip(got, flat))
    limit = TOLERANCE * (1 + max(abs(v) for v in flat))
    print(f"{name}: largest difference {worst:.3g}, limit {limit:.3g}, {'within' if worst <= limit else 'OUTSIDE'}")
    return worst <= limit


def rows_of(a, n, width):
    return [list(a[i * width:(i + 1) * width]) for i in range(n)]


def main():
    if len(sys.argv) < 4 or sys.argv[1] not in ("ranges", "compare") or (sys.argv[1] == "compare") != (len(sys.argv) == 5):
        print(__doc__.strip().splitlines()[6].strip(), file=sys.stderr)
        return 2
    directory, out = sys.argv[2], pathlib.Path(sys.argv[3])
    if sys.argv[1] == "ranges":
        path, base, head = header(directory)
        lines = [f"{k} {base + v['data_offsets'][0]} {base + v['data_offsets'][1]}" for k, v in sorted(head.items()) if k != "__metadata__"]
        (out / "ranges.txt").write_text("\n".join(lines) + "\n")
        print(f"{len(lines)} tensor ranges")
        return 0
    raw = pathlib.Path(sys.argv[4]).read_bytes()
    pcm = struct.unpack(f"<{len(raw) // 2}h", raw)
    wt = Weights(directory)
    feats, frames = features(pcm)
    enc = (((frames + 1) // 2 + 1) // 2 + 1) // 2
    got = load(out, "features.f32", MELS * frames)
    ok = gap(f"log-mel features, {frames} frames", got, feats)
    coax_feats = rows_of(got, MELS, frames)
    ok = gap(f"subsampling, {enc} frames", load(out, "subsample.f32", enc * D), subsample(wt, coax_feats, frames)) and ok
    ok = gap("encoder layer 0", load(out, "layer0.f32", enc * D), layer(wt, 0, rows_of(load(out, "subsample.f32", enc * D), enc, D))) and ok
    ok = gap("encoder layer 1", load(out, "layer1.f32", enc * D), layer(wt, 1, rows_of(load(out, "layer0.f32", enc * D), enc, D))) and ok
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
