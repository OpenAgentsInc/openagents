#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 100's composed oracle: the whole 40-layer V4.1 Flash decode stack
on the chapter's small synthetic weights, built only from the per-chapter
Python oracles (91 router, 92 sparse attention, 93 hyper-connections, 94
Engram hashing, 95 experts, 98 cache packing) plus the glue written here.

    python3 -I psionic/tests/deepseek_flash_oracle.py [--write]

It prints the routed experts, the selected compressed positions and the
logits of each step, with the smallest router margin; --write replaces
psionic/fixtures/deepseek/flash-goldens.coax. Tooling only; never shipped.
"""
import importlib.util
import math
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / 'psionic/fixtures/deepseek/flash-goldens.coax'


def load(name):
    spec = importlib.util.spec_from_file_location('owned_' + name, ROOT / f'psionic/tests/{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


sparse = load('deepseek_sparse_oracle')
hyper = load('deepseek_hyper_oracle')
router = load('deepseek_router_oracle')
expert = load('deepseek_expert_oracle')
engram = load('deepseek_engram_oracle')
cache = load('deepseek_cache_oracle')

D, HC, H, W, R, WIN, QL, OL, G, IH = 32, 4, 2, 32, 8, 3, 32, 32, 2, 2
E, K, F, V, L, CAP, TOPK, BLOCK, BLOCKS, TR = 8, 2, 32, 64, 40, 8, 3, 3, 2, 16
TOKENS = [3, 17, 42, 5, 61, 17, 8]


def f32(x): return struct.unpack('<f', struct.pack('<f', float(x)))[0]
def bits32(x): return struct.unpack('<I', struct.pack('<f', f32(x)))[0]
def frombf(b): return struct.unpack('<f', struct.pack('<I', b << 16))[0]
def bf(x): return sparse.bf(x)


# The same integer generator as flash.coax, written again here.
def hsh(seed, i):
    x = (seed * 2654435761 + i * 40503 + 12345) & 0xffffffff
    x = ((x ^ (x >> 13)) * 1274126177) & 0xffffffff
    return x ^ (x >> 16)


def seed(layer, ident): return 1000 * (layer + 1) + ident
def bf16(s, i): return bits32((hsh(s, i) % 257 - 128) / 256) >> 16
def norm(s, i): return bits32(1 + (hsh(s, i) % 17 - 8) / 64) >> 16
def unit(s, i, shift): return f32((hsh(s, i) % 2001 - 1000) / (1 << shift))


MATRICES = {}


def fp8(s, rows, cols):
    key = ('fp8', s, rows, cols)
    if key not in MATRICES:
        codes = []
        for i in range(rows * cols):
            h = hsh(s, i)
            codes.append(h % 56 + 8 + (128 if (h >> 8) % 2 else 0))
        blocks = cols // 32
        scales = [121 + hsh(s + 500000, b) % 3 for b in range(((rows + 31) // 32) * blocks)]
        values = [expert.scalar('fp8', codes[i], scales[(i // cols // 32) * blocks + (i % cols) // 32]) for i in range(rows * cols)]
        MATRICES[key] = dict(kind='fp8', rows=rows, cols=cols, values=values)
    return MATRICES[key]


def fp4(s, rows, cols):
    key = ('fp4', s, rows, cols)
    if key not in MATRICES:
        data = [hsh(s, i) % 256 for i in range(rows * cols // 2)]
        scales = [122 + hsh(s + 500000, b) % 3 for b in range(rows * (cols // 32))]
        values = [expert.scalar('fp4', (data[i // 2] >> (4 * (i % 2))) & 15, scales[i // 32]) for i in range(rows * cols)]
        MATRICES[key] = dict(kind='fp4', rows=rows, cols=cols, values=values)
    return MATRICES[key]


def project(m, x): return expert.project(m, x)


def rms(x, s):
    return [frombf(b) for b in sparse.normalized(x, [norm(s, i) for i in range(len(x))])]


def hyper_weights(layer, ident):
    projection = [unit(seed(layer, ident), i, 15) for i in range(96 * D)]
    rows = [projection[r * 4 * D:(r + 1) * 4 * D] for r in range(24)]
    base = [unit(seed(layer, ident + 1), i, 10) for i in range(24)]
    scale = [f32((hsh(seed(layer, ident + 2), i) % 9) / 8 + 0.25) for i in range(3)]
    return rows, scale, base


def links(layer):
    ratio = 2 if 2 <= layer < 20 else 1 if 20 <= layer < 40 else 0
    kv = max((o for o in [2, 8, 14, 20] if o <= layer), default=-1) if ratio else -1
    index = max((o for o in [2, 8, 14, 20, 24, 28, 32, 36] if o <= layer), default=-1) if ratio else -1
    return ratio, kv, index, (20 if ratio and layer >= 20 else -1)


class State:
    def __init__(self):
        self.window = {l: [None] * WIN for l in range(L)}
        self.comp = {l: [] for l in range(L)}
        self.keys = {l: [] for l in range(L)}
        self.old = {l: ([0.0] * W, [0.0] * W) for l in range(L)}
        self.picks = {}
        self.scores = {}
        self.mask = [0] * CAP


def attention(st, layer, p, x):
    ratio, kv_owner, index_owner, candidate = links(layer)
    xn = rms(x, seed(layer, 60))
    qr = project(fp8(seed(layer, 1), QL, D), xn)
    qrn = rms(qr, seed(layer, 62))
    q = sparse.rotate(bool(ratio), p, H, W, R, False, [bf(v) for v in project(fp8(seed(layer, 2), H * W, QL), qrn)])
    kv = rms(project(fp8(seed(layer, 3), W, D), xn), seed(layer, 63))
    row = sparse.rotate(bool(ratio), p, 1, W, R, False, [bf(v) for v in kv])
    st.window[layer][p % WIN] = sparse.packed(0, [[frombf(b) for b in row]], W)['values']
    ids = []
    if ratio:
        visible = (p + 1) // ratio
        if kv_owner == layer:
            ckv = project(fp8(seed(layer, 7), W, D), xn)
            gate = project(fp8(seed(layer, 8), W, D), xn)
            old_kv, old_gate = st.old[layer]
            ready = ratio == 1 or p % 2 == 1
            if ratio == 1:
                pooled = ckv
            elif ready:
                pooled = []
                for i in range(W):
                    top = max(gate[i], old_gate[i])
                    a = math.exp(old_gate[i] - top); b = math.exp(gate[i] - top)
                    pooled.append(f32((old_kv[i] * a + ckv[i] * b) / (a + b)))
            st.old[layer] = ([0.0] * W, [0.0] * W) if ready or ratio == 1 else (ckv, gate)
            if ready:
                first = p + 1 - ratio
                latent = sparse.normalized(pooled, [norm(seed(layer, 64), i) for i in range(W)])
                key = sparse.normalized([frombf(b) for b in latent], [bf(1)] * W)
                key = sparse.rotate(True, first, 1, W, R, False, key)
                st.keys[layer].append(sparse.packed(2, [[frombf(b) for b in key]], W)['values'])
                latent = sparse.rotate(True, first, 1, W, R, False, latent)
                st.comp[layer].append(sparse.packed(1, [[frombf(b) for b in latent]], W)['values'])
        if index_owner == layer:
            st.picks[layer] = []
            st.scores[layer] = [0.0] * CAP
            if visible:
                iq = sparse.rotate(True, p, IH, W, R, False, [bf(v) for v in project(fp8(seed(layer, 9), IH * W, QL), qrn)])
                iq = sparse.packed(2, [[frombf(b) for b in iq[h * W:(h + 1) * W]] for h in range(IH)], W)['values']
                weights = [bf(v) for v in project(fp8(seed(layer, 10), IH, D), xn)]
                keys = [v for r in st.keys[kv_owner][:visible] for v in r]
                scores = sparse.index_values(IH, W, iq, keys, weights) + [0.0] * (CAP - visible)
                st.scores[layer] = scores
                if layer == 20:
                    st.mask = sparse.candidates(scores, visible, BLOCK, BLOCKS)
                mask = st.mask if candidate >= 0 else [1] * CAP
                st.picks[layer] = sparse.select(scores, visible, mask, TOPK)
        ids = st.picks[index_owner]
    keys = [st.window[layer][t % WIN] for t in range(max(0, p - WIN + 1), p + 1)]
    keys += [st.comp[kv_owner][i] for i in ids]
    sinks = [unit(seed(layer, 82), h, 9) for h in range(H)]
    answer = sparse.attention_values([frombf(b) for b in q], keys, sinks, W)
    o = [frombf(b) for b in sparse.rotate(bool(ratio), p, H, W, R, True, answer)]
    low = []
    for g in range(G):
        low += project(fp8(seed(layer, 4 + g), OL, W), o[g * 32:(g + 1) * 32])
    seen = st.scores[index_owner] if ratio else [0.0] * CAP
    return project(fp8(seed(layer, 6), D, G * OL), low), ids, seen


def moe(layer, x):
    xn = rms(x, seed(layer, 61))
    logits = [f32(math.fsum(frombf(bf16(seed(layer, 80), e * D + j)) * xn[j] for j in range(D))) for e in range(E)]
    bias = [unit(seed(layer, 81), e, 12) for e in range(E)]
    route = router.route(dict(profile='v41', logits=logits, bias=bias, k=K, groups=1, selected_groups=1, scale=1.5))
    outs = []
    for i, e in zip(route['ids'], route['weights']):
        s = seed(layer, 20 + 3 * i)
        ex = dict(gate=fp4(s, F, D), up=fp4(s + 1, F, D), down=fp4(s + 2, D, F))
        outs.append(expert.execute(ex, xn, f32(e), 10)['output'])
    combined = [f32(math.fsum(o[j] for o in outs)) for j in range(D)]
    shared = dict(gate=fp8(seed(layer, 50), F, D), up=fp8(seed(layer, 51), F, D), down=fp8(seed(layer, 52), D, F))
    s = expert.execute(shared, xn, 1, 10)['output']
    return [f32(c + v) for c, v in zip(combined, s)], route['ids'], route['margin']


def engram_layer(layer, ids, x):
    rows = []
    for i in ids:
        r = i % TR
        codes = []
        for b in range(264):
            h = hsh(seed(layer, 91), r * 264 + b)
            codes.append(h % 56 + 8 + 128 * ((h >> 8) % 2) if b < 256 else 124 + h % 3)
        for col in range(256):
            code, scale = codes[col], codes[256 + col // 32]
            rows.append(frombf(cache.bf_round(f32(expert.scalar('fp8', code, scale)))))
    kv = project(fp8(seed(layer, 90), 160, 6144), rows)
    return engram_mix(layer, kv, x)


# The per-stream gate: the value row scaled by a sigmoid of the signed root
# of the normalized key-stream agreement.
def engram_mix(layer, kv, x):
    out = list(x)
    for c in range(HC):
        h = x[c * D:(c + 1) * D]; key = kv[c * D:(c + 1) * D]
        w = [unit(seed(layer, 92), c * D + j, 12) + 1 for j in range(D)]
        rstd = (1 / math.sqrt(math.fsum(v * v for v in h) / D + 1e-20)) * (1 / math.sqrt(math.fsum(v * v for v in key) / D + 1e-20))
        d = math.fsum(a * b * k for a, b, k in zip(h, w, key)) * rstd / math.sqrt(D)
        z = math.copysign(math.sqrt(max(abs(d), 1e-6)), d)
        gate = 1 / (1 + math.exp(-z))
        for j in range(D):
            out[c * D + j] = f32(h[j] + gate * kv[4 * D + j])
    return out


def run(layers=L, use_engram=True, tokens=TOKENS, first=0):
    mapping = [(i * 37 + 11) % 99092 for i in range(V)]
    hashes = engram.hashes(tokens, [1] * len(tokens), mapping)
    st = State()
    routes, picks, logits, margins, scores = [], [], [], [], []
    for p, token in enumerate(tokens):
        x = [frombf(bf16(7, token * D + j)) for j in range(D)] * HC
        pre = [1.0, 0.0, 0.0, 0.0]
        for layer in range(first, layers):
            if use_engram and layer in (1, 14):
                slot = 0 if layer == 1 else 1
                x = engram_layer(layer, hashes[p * 48 + slot * 24:p * 48 + slot * 24 + 24], x)
            rows, scale, base = hyper_weights(layer, 70)
            a = hyper.split(hyper.project(x, rows), scale, base)
            y, ids, seen = attention(st, layer, p, hyper.collapse(x, pre, D))
            x = hyper.expand(y, x, a, D)
            rows, scale, base = hyper_weights(layer, 73)
            b = hyper.split(hyper.project(x, rows), scale, base)
            y, chosen, margin = moe(layer, hyper.collapse(x, a['pre'], D))
            x = hyper.expand(y, x, b, D)
            pre = b['pre']
            routes += chosen
            picks += ids + [-1] * (TOPK - len(ids))
            scores += seen
            margins.append(margin)
        hn = rms(hyper.collapse(x, pre, D), 9)
        logits += [f32(math.fsum(frombf(bf16(8, v * D + j)) * hn[j] for j in range(D))) for v in range(V)]
        print(f'step {p}: token {token}, top logit id {max(range(V), key=lambda v: logits[p * V + v])}', file=sys.stderr)
    return routes, picks, logits, margins, scores


def emit(routes, picks, logits, margins, scores, prefix='DVG'):
    s = ''
    s += f'{prefix}_ROUTES :: [{len(routes)}]int.[' + ','.join(map(str, routes)) + '];\n'
    s += f'{prefix}_PICKS :: [{len(picks)}]int.[' + ','.join(map(str, picks)) + '];\n'
    s += f'{prefix}_LOGITS :: [{len(logits)}]u32.[' + ','.join(f'0x{bits32(v):08x}' for v in logits) + '];\n'
    s += f'{prefix}_MIN_MARGIN :: {min(margins)!r};\n'
    if prefix != 'DVG':
        s += f'{prefix}_SCORES :: [{len(scores)}]u32.[' + ','.join(f'0x{bits32(v):08x}' for v in scores) + '];\n'
    return s


def mix_case():
    x = [0.0 if i < D else unit(5, i, 10) for i in range(HC * D)]
    kv = [0.0 if 2 * D <= i < 3 * D else unit(6, i, 10) for i in range(160)]
    return x, kv, engram_mix(1, kv, x)


def main():
    routes, picks, logits, margins, scores = run()
    text = '// Chapter 100 composed-oracle goldens (psionic/tests/deepseek_flash_oracle.py).\n'
    text += f'DVG_TOKENS :: [{len(TOKENS)}]int.[' + ','.join(map(str, TOKENS)) + '];\n'
    text += emit(routes, picks, logits, margins, scores)
    text += '// Layers 0-20 without Engram, two steps: what the compile-time check affords.\n'
    text += emit(*run(21, False, TOKENS[:2]), prefix='DVG21')
    text += '// Layers 20-39 without Engram, two steps: ratio-1 sharing and every indexer.\n'
    text += emit(*run(40, False, TOKENS[:2], 20), prefix='DVG2040')
    text += '// Layers 20-24 for eight tokens: the window wraps and the candidate mask drops a block.\n'
    text += emit(*run(25, False, TOKENS + [29], 20), prefix='DVG2024')
    text += '// Layers 0-1 with Engram for two steps, and layer 14 alone for one.\n'
    text += emit(*run(2, True, TOKENS[:2]), prefix='DVGE1')
    text += emit(*run(15, True, TOKENS[:1], 14), prefix='DVGE14')
    text += '// The Engram gate alone on a fixed input: stream 0 and key stream 2 are zero.\n'
    x, kv, mixed = mix_case()
    text += 'DVGM_OUT :: [128]u32.[' + ','.join(f'0x{bits32(v):08x}' for v in mixed) + '];\n'
    print(f'smallest router margin {min(margins)!r}; {len(logits)} logits')
    if '--write' in sys.argv[1:]:
        FIXTURE.write_text(text)
    elif not FIXTURE.exists() or FIXTURE.read_text() != text:
        print('flash-goldens.coax differs from the oracle; run with --write', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
