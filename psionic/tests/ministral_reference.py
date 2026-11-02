"""Chapter 81's independent references for Ministral-3-3B-Instruct-2512-BF16.

Written from the published architecture (params.json, Mistral's reference
description of the decoder, and tiktoken's byte-pair rule), not from Coax and
not from any downloaded code. Pure Python, float64. Tooling only.

    python3 -I psionic/tests/ministral_reference.py pin MODEL_DIRECTORY [--write]
        hash the pinned files; compare (or write) psionic/fixtures/ministral-manifest.coax
    python3 -I psionic/tests/ministral_reference.py tekken MODEL_DIRECTORY [--write]
        tokenize the fixture strings with tekken.json; compare (or write) psionic/fixtures/ministral-tekken.coax
    python3 -I psionic/tests/ministral_reference.py tiny [--write]
        the synthetic two-layer model and the 3B rotary frequencies; compare (or write) psionic/fixtures/ministral-tiny.coax
    python3 -I psionic/tests/ministral_reference.py slice-tokens MODEL_DIRECTORY OUT_DIRECTORY
        write OUT_DIRECTORY/tokens.txt, the chat-format prompt the slices run
    python3 -I psionic/tests/ministral_reference.py slices MODEL_DIRECTORY OUT_DIRECTORY
        recompute the original-weight slices the Coax harness wrote to OUT_DIRECTORY and compare

MODEL_DIRECTORY holds consolidated.safetensors, tekken.json and params.json
from https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512-BF16 at
REVISION. The script reads the safetensors header and the byte ranges of the
tensors it needs; it never imports or runs anything from the download.
"""
import base64
import hashlib
import json
import math
import pathlib
import re
import struct
import sys
from array import array

REVISION = "b6d637bef2393152b3da2b2fde72eecdee30557e"
FILES = {
    "consolidated.safetensors": (7698234952, "722430146632596c6c41137b2e582095ed01a50342932b035cb8662f7855a340"),
    "tekken.json": (16753784, "600bb27946565481ecf51ba8aee252e49b9a68507866080ac9c30185bb312843"),
    "params.json": (1096, None),
}
ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures"

# The published text decoder (params.json).
DIM, LAYERS, HEADS, KV_HEADS, HEAD_DIM, HIDDEN, VOCAB = 3072, 26, 32, 8, 128, 9216, 131072
THETA, EPS, FACTOR, ORIGINAL, BETA_FAST, BETA_SLOW, LLAMA4_BETA = 1e6, 1e-5, 16.0, 16384, 32.0, 1.0, 0.1
SPECIALS = 1000

TEKKEN_STRINGS = [
    "Hello, world!",
    "What is the capital of France?",
    "  leading spaces\n\nand 2025 numbers",
    "CamelCaseWord IPAddress x/y\r\n",
    "don't stop",
    "tabs\tand trailing   ",
    "a+b=c; // comment",
    "",
]
# The prompt the original-weight slices run.
SLICE_PROMPT = "What is the capital of France?"


def sha256(path):
    d = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 24), b""):
            d.update(chunk)
    return d.hexdigest()


def coax_string(s):
    out = []
    for ch in s:
        c = ord(ch)
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t")
        elif 32 <= c < 127:
            out.append(ch)
        else:
            raise ValueError(f"fixture text must be printable ASCII: {s!r}")
    return '"' + "".join(out) + '"'


def compare_or_write(path, text, write, what):
    if write:
        path.write_text(text)
        print(f"wrote {path}")
        return 0
    if not path.exists() or path.read_text() != text:
        print(f"{path} differs from {what}", file=sys.stderr)
        return 1
    print(f"{path.name} matches {what}")
    return 0


def header(directory):
    with open(pathlib.Path(directory) / "consolidated.safetensors", "rb") as f:
        n = struct.unpack("<Q", f.read(8))[0]
        if not 2 <= n <= 1 << 20:
            raise SystemExit(f"refused: header of {n} bytes")
        return 8 + n, json.loads(f.read(n))


# ---- pin -------------------------------------------------------------------

def pin(directory, write):
    directory = pathlib.Path(directory)
    shas = {}
    for name, (size, digest) in FILES.items():
        path = directory / name
        if path.stat().st_size != size:
            print(f"refused: {path} is {path.stat().st_size} bytes, the pin says {size}", file=sys.stderr)
            return 1
        got = sha256(path)
        if digest is not None and got != digest:
            print(f"refused: {name} sha256 {got} is not the pinned {digest}", file=sys.stderr)
            return 1
        shas[name] = got
    params = json.loads((directory / "params.json").read_text())
    expect = {"dim": DIM, "n_layers": LAYERS, "n_heads": HEADS, "n_kv_heads": KV_HEADS, "head_dim": HEAD_DIM,
              "hidden_dim": HIDDEN, "vocab_size": VOCAB, "rope_theta": THETA, "norm_eps": EPS, "tied_embeddings": True}
    for k, v in expect.items():
        if params.get(k) != v:
            print(f"refused: params.json {k} is {params.get(k)!r}, expected {v!r}", file=sys.stderr)
            return 1
    yarn = params["yarn"]
    if (yarn["factor"], yarn["original_max_position_embeddings"], yarn["beta"], yarn["alpha"], yarn["apply_scale"]) != (FACTOR, ORIGINAL, BETA_FAST, BETA_SLOW, False):
        print("refused: params.json yarn differs", file=sys.stderr)
        return 1
    if params["llama_4_scaling"] != {"original_max_position_embeddings": ORIGINAL, "beta": LLAMA4_BETA}:
        print("refused: params.json llama_4_scaling differs", file=sys.stderr)
        return 1
    _, h = header(directory)
    lines = []
    for name in sorted(h):
        if name == "__metadata__":
            continue
        t = h[name]
        lines.append(f"{name} {t['dtype']} {','.join(str(d) for d in t['shape'])}")
    text = "\\n".join(lines) + "\\n"
    out = (
        "// Chapter 81's checkpoint inventory, written by psionic/tests/ministral_reference.py pin\n"
        f"// from mistralai/Ministral-3-3B-Instruct-2512-BF16 at {REVISION} (Apache-2.0),\n"
        f"// consolidated.safetensors, {FILES['consolidated.safetensors'][0]} bytes, sha256 {shas['consolidated.safetensors']};\n"
        f"// tekken.json, {FILES['tekken.json'][0]} bytes, sha256 {shas['tekken.json']};\n"
        f"// params.json, {FILES['params.json'][0]} bytes, sha256 {shas['params.json']}.\n"
        "// One line a tensor, sorted by name: name, dtype, dimensions. No weights.\n"
        f'MS_PINNED_REVISION :: "{REVISION}";\n'
        f'MS_PINNED_SHA256 :: "{shas["consolidated.safetensors"]}";\n'
        f"MS_PINNED_TENSORS :: {len(lines)};\n"
        f'MS_PINNED_MANIFEST :: "{text}";\n'
    )
    return compare_or_write(FIXTURES / "ministral-manifest.coax", out, write, "the pinned checkpoint's header")


# ---- tekken ----------------------------------------------------------------

# Tekken's split pattern restricted to ASCII text: \p{Lu} is A-Z, \p{Ll} is
# a-z, \p{Lt}, \p{Lm}, \p{Lo} and \p{M} have no ASCII members, \p{N} is 0-9
# and \s is Unicode White_Space, whose ASCII members are tab to carriage
# return and space.
S = r"\t\n\x0b\x0c\r "
ASCII_PATTERN = re.compile(
    r"[^\r\nA-Za-z0-9]?[A-Z]*[a-z]+"
    r"|[^\r\nA-Za-z0-9]?[A-Z]+[a-z]*"
    r"|[0-9]"
    rf"| ?[^{S}A-Za-z0-9]+[\r\n/]*"
    rf"|[{S}]*[\r\n]+"
    rf"|[{S}]+(?![^{S}])"
    rf"|[{S}]+"
)
TEKKEN_PATTERN = (r"[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]*[\p{Ll}\p{Lm}\p{Lo}\p{M}]+|[^\r\n\p{L}\p{N}]?"
                  r"[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]+[\p{Ll}\p{Lm}\p{Lo}\p{M}]*|\p{N}| ?[^\s\p{L}\p{N}]+[\r\n/]*|\s*[\r\n]+|\s+(?!\S)|\s+")


def pieces(text):
    assert all(ord(c) < 128 for c in text)
    out, at = [], 0
    while at < len(text):
        m = ASCII_PATTERN.match(text, at)
        assert m and m.end() > at
        out.append(text[at:m.end()].encode())
        at = m.end()
    return out


def bpe(ranks, piece):
    """tiktoken's rule: merge the adjacent pair whose joined bytes have the
    lowest rank, the leftmost on a tie, until no joined pair has a rank."""
    if piece in ranks:
        return [ranks[piece]]
    parts = [piece[i:i + 1] for i in range(len(piece))]
    while True:
        best, at = None, -1
        for i in range(len(parts) - 1):
            r = ranks.get(parts[i] + parts[i + 1])
            if r is not None and (best is None or r < best):
                best, at = r, i
        if best is None:
            break
        parts[at:at + 2] = [parts[at] + parts[at + 1]]
    return [ranks[p] for p in parts]


def load_tekken(directory):
    data = json.loads((pathlib.Path(directory) / "tekken.json").read_text())
    config = data["config"]
    if config["pattern"] != TEKKEN_PATTERN or config["default_num_special_tokens"] != SPECIALS or config["default_vocab_size"] != VOCAB:
        raise SystemExit("refused: tekken.json config differs from the pinned Tekken v13")
    vocab = data["vocab"][: VOCAB - SPECIALS]
    ranks = {}
    for i, entry in enumerate(vocab):
        assert entry["rank"] == i
        ranks[base64.b64decode(entry["token_bytes"])] = i
    assert all(ranks[bytes([b])] == b for b in range(256))
    specials = [s["token_str"] for s in data["special_tokens"][:20]]
    assert specials[:5] == ["<unk>", "<s>", "</s>", "[INST]", "[/INST]"] and specials[17:19] == ["[SYSTEM_PROMPT]", "[/SYSTEM_PROMPT]"]
    return ranks


def encode(ranks, text):
    return [SPECIALS + r for p in pieces(text) for r in bpe(ranks, p)]


def slice_tokens(directory, out):
    tokens = [1, 3] + encode(load_tekken(directory), SLICE_PROMPT) + [4]
    (pathlib.Path(out) / "tokens.txt").write_text(" ".join(map(str, tokens)) + "\n")
    print(f"{len(tokens)} tokens: {tokens}")
    return 0


def tekken(directory, write):
    ranks = load_tekken(directory)
    # The fixture table is Tekken restricted to the fixture's pieces: every
    # entry whose bytes occur inside some piece. Byte-pair merging of a piece
    # only ever forms substrings of that piece, so the restriction encodes the
    # fixture strings exactly as the full table does.
    subset = {}
    expected = []
    for text in TEKKEN_STRINGS:
        ids = []
        for p in pieces(text):
            for i in range(len(p)):
                for j in range(i + 1, len(p) + 1):
                    if p[i:j] in ranks:
                        subset[p[i:j]] = ranks[p[i:j]]
            ids += [SPECIALS + r for r in bpe(ranks, p)]
        expected.append(ids)
    entries = sorted(subset.items())
    lines = [
        "// Tekken v13 restricted to chapter 81's fixture strings, written by",
        f"// psionic/tests/ministral_reference.py tekken from tekken.json at {REVISION}",
        f"// (sha256 {FILES['tekken.json'][1]}). Each entry is a byte string and its",
        "// original rank; token ids are rank + 1000. The ids are tiktoken's rule run",
        "// in Python over the full 130,072-entry table.",
        f"MS_TEKKEN_ENTRIES :: {len(entries)};",
        f"MS_TEKKEN_BYTES :: [{len(entries)}]string.[" + ",".join(coax_string(b.decode()) for b, _ in entries) + "];",
        f"MS_TEKKEN_RANKS :: [{len(entries)}]int.[" + ",".join(str(r) for _, r in entries) + "];",
        f"MS_TEKKEN_TEXTS :: [{len(TEKKEN_STRINGS)}]string.[" + ",".join(coax_string(t) for t in TEKKEN_STRINGS) + "];",
    ]
    flat, starts = [], [0]
    for ids in expected:
        flat += ids
        starts.append(len(flat))
    lines.append(f"MS_TEKKEN_STARTS :: [{len(starts)}]int.[" + ",".join(map(str, starts)) + "];")
    lines.append(f"MS_TEKKEN_IDS :: [{len(flat)}]int.[" + ",".join(map(str, flat)) + "];")
    chat = [1, 17] + expected[0] + [18, 3] + expected[1] + [4]
    lines.append(f"MS_TEKKEN_CHAT :: [{len(chat)}]int.[" + ",".join(map(str, chat)) + "];")
    return compare_or_write(FIXTURES / "ministral-tekken.coax", "\n".join(lines) + "\n", write, "Tekken on the fixture strings")


# ---- the decoder -----------------------------------------------------------

def rope_frequencies(head_dim):
    """YaRN (factor 16 over 16,384 original positions, beta 32 and 1) on
    theta 1e6, per params.json; apply_scale is false, so no magnitude scale."""
    half = head_dim // 2
    extrapolated = [THETA ** (-2 * i / head_dim) for i in range(half)]

    def correction(rotations):
        return head_dim * math.log(ORIGINAL / (rotations * 2 * math.pi)) / (2 * math.log(THETA))

    low = max(math.floor(correction(BETA_FAST)), 0)
    high = min(math.ceil(correction(BETA_SLOW)), head_dim - 1)
    if low == high:
        high += 0.001
    out = []
    for i in range(half):
        ramp = min(max((i - low) / (high - low), 0.0), 1.0)
        keep = 1.0 - ramp  # weight on the original (extrapolated) frequency
        out.append(extrapolated[i] / FACTOR * (1 - keep) + extrapolated[i] * keep)
    return out


def query_scale(position):
    return 1 + LLAMA4_BETA * math.log(1 + position // ORIGINAL)


def rms(x, w):
    s = math.sqrt(sum(v * v for v in x) / len(x) + EPS)
    return [v / s * g for v, g in zip(x, w)]


def matvec(w, rows, cols, x):
    return [_dot(w, r * cols, cols, x) for r in range(rows)]


def _dot(w, start, cols, x):
    return math.fsum(a * b for a, b in zip(w[start:start + cols], x))


def rotate(vec, heads, head_dim, position, freqs):
    out = list(vec)
    for h in range(heads):
        for i in range(head_dim // 2):
            a, b = vec[h * head_dim + 2 * i], vec[h * head_dim + 2 * i + 1]
            angle = position * freqs[i]
            c, s = math.cos(angle), math.sin(angle)
            out[h * head_dim + 2 * i] = a * c - b * s
            out[h * head_dim + 2 * i + 1] = a * s + b * c
    return out


def layer(x_rows, w, shape, freqs):
    d, heads, kv, hd, hidden = shape
    T = len(x_rows)
    qs, ks, vs = [], [], []
    for t, x in enumerate(x_rows):
        h = rms(x, w["attention_norm"])
        q = rotate(matvec(w["wq"], heads * hd, d, h), heads, hd, t, freqs)
        q = [v * query_scale(t) for v in q]
        qs.append(q)
        ks.append(rotate(matvec(w["wk"], kv * hd, d, h), kv, hd, t, freqs))
        vs.append(matvec(w["wv"], kv * hd, d, h))
    out = []
    group = heads // kv
    for t in range(T):
        attended = []
        for hh in range(heads):
            g = hh // group
            q = qs[t][hh * hd:(hh + 1) * hd]
            scores = [math.fsum(a * b for a, b in zip(q, ks[u][g * hd:(g + 1) * hd])) / math.sqrt(hd) for u in range(t + 1)]
            top = max(scores)
            e = [math.exp(s - top) for s in scores]
            z = math.fsum(e)
            for j in range(hd):
                attended.append(math.fsum(e[u] / z * vs[u][g * hd + j] for u in range(t + 1)))
        o = matvec(w["wo"], d, heads * hd, attended)
        x = [a + b for a, b in zip(x_rows[t], o)]
        h = rms(x, w["ffn_norm"])
        gate = matvec(w["w1"], hidden, d, h)
        up = matvec(w["w3"], hidden, d, h)
        act = [g / (1 + math.exp(-g)) * u for g, u in zip(gate, up)]
        m = matvec(w["w2"], d, hidden, act)
        out.append([a + b for a, b in zip(x, m)])
    return out


# The synthetic model: Ministral's structure at width 8, 4 query heads, 2 KV
# heads of 4, hidden 12, vocabulary 16 and 2 layers, with weights from a fixed
# formula that both sides compute. Every weight is a multiple of 1/32 in
# [-1/4, 1/4], exact in BF16.
TINY = dict(d=8, heads=4, kv=2, hd=4, hidden=12, vocab=16, layers=2)
TINY_TOKENS = [1, 3, 7, 12, 4]


def tiny_value(k, i):
    return ((i * 7 + k * 5) % 17 - 8) / 32


def tiny_norm(k, i):
    return 1 + ((i + k) % 5 - 2) / 16


def tiny_weights():
    t = TINY
    d, hd = t["d"], t["hd"]
    sizes = [("wq", t["heads"] * hd * d), ("wk", t["kv"] * hd * d), ("wv", t["kv"] * hd * d), ("wo", d * t["heads"] * hd),
             ("w1", t["hidden"] * d), ("w2", d * t["hidden"]), ("w3", t["hidden"] * d)]
    layers = []
    for l in range(t["layers"]):
        w = {}
        for j, (name, n) in enumerate(sizes):
            k = 1 + l * 7 + j
            w[name] = [tiny_value(k, i) for i in range(n)]
        w["attention_norm"] = [tiny_norm(100 + 2 * l, i) for i in range(d)]
        w["ffn_norm"] = [tiny_norm(101 + 2 * l, i) for i in range(d)]
        layers.append(w)
    embed = [tiny_value(0, i) for i in range(t["vocab"] * d)]
    norm = [tiny_norm(99, i) for i in range(d)]
    return embed, layers, norm


def coax_floats(values):
    return ",".join(repr(float(v)) for v in values)


def tiny(write):
    t = TINY
    d = t["d"]
    embed, layers, norm = tiny_weights()
    freqs = rope_frequencies(t["hd"])
    x = [embed[tok * d:(tok + 1) * d] for tok in TINY_TOKENS]
    after = []
    for w in layers:
        x = layer(x, w, (d, t["heads"], t["kv"], t["hd"], t["hidden"]), freqs)
        after.append([v for row in x for v in row])
    h = rms(x[-1], norm)
    logits = [math.fsum(a * b for a, b in zip(embed[r * d:(r + 1) * d], h)) for r in range(t["vocab"])]
    big = rope_frequencies(HEAD_DIM)
    picks = [0, 1, 10, 20, 21, 30, 40, 63]
    lines = [
        "// Chapter 81's float64 references, written by psionic/tests/ministral_reference.py tiny.",
        f"MS_TINY_TOKENS :: [{len(TINY_TOKENS)}]int.[" + ",".join(map(str, TINY_TOKENS)) + "];",
        f"MS_TINY_LAYER0 :: [{len(after[0])}]f64.[" + coax_floats(after[0]) + "];",
        f"MS_TINY_LAYER1 :: [{len(after[1])}]f64.[" + coax_floats(after[1]) + "];",
        f"MS_TINY_LOGITS :: [{len(logits)}]f64.[" + coax_floats(logits) + "];",
        f"MS_ROPE_PICKS :: [{len(picks)}]int.[" + ",".join(map(str, picks)) + "];",
        f"MS_ROPE_FREQS :: [{len(picks)}]f64.[" + coax_floats(big[i] for i in picks) + "];",
    ]
    return compare_or_write(FIXTURES / "ministral-tiny.coax", "\n".join(lines) + "\n", write, "the float64 reference")


# ---- original-weight slices -------------------------------------------------

def load(directory, start, h, name, first=0, count=None):
    t = h[name]
    assert t["dtype"] == "BF16"
    n = 1
    for v in t["shape"]:
        n *= v
    count = n - first if count is None else count
    with open(pathlib.Path(directory) / "consolidated.safetensors", "rb") as f:
        f.seek(start + t["data_offsets"][0] + 2 * first)
        raw = f.read(2 * count)
    wide = bytearray(4 * count)
    wide[2::4] = raw[0::2]
    wide[3::4] = raw[1::2]
    return array("f", bytes(wide)).tolist()


def read_f32(path, n):
    data = pathlib.Path(path).read_bytes()
    if len(data) != 4 * n:
        raise SystemExit(f"{path}: {len(data)} bytes, expected {4 * n}")
    return array("f", data).tolist()


# Tolerances frozen before the first comparison: the embedding gather is
# exact; a layer and the readout chunk may differ from the float64 reference
# by at most 1e-4 of the slice's largest magnitude (Coax keeps f32 values
# between float64 sums).
LAYER_TOLERANCE = 1e-4
READOUT_ROWS = 4096


def slices(directory, out):
    tokens = [int(v) for v in (pathlib.Path(out) / "tokens.txt").read_text().split()]
    start, h = header(directory)
    T = len(tokens)
    embed = []
    for tok in tokens:
        embed += load(directory, start, h, "tok_embeddings.weight", tok * DIM, DIM)
    got = read_f32(pathlib.Path(out) / "embed.f32", T * DIM)
    if got != embed:
        print("embedding slice differs", file=sys.stderr)
        return 1
    print(f"embed: {T} tokens, exact")
    w = {k: load(directory, start, h, f"layers.0.attention.{k}.weight") for k in ("wq", "wk", "wv", "wo")}
    w.update({k: load(directory, start, h, f"layers.0.feed_forward.{k}.weight") for k in ("w1", "w2", "w3")})
    w["attention_norm"] = load(directory, start, h, "layers.0.attention_norm.weight")
    w["ffn_norm"] = load(directory, start, h, "layers.0.ffn_norm.weight")
    ref = layer([embed[t * DIM:(t + 1) * DIM] for t in range(T)], w, (DIM, HEADS, KV_HEADS, HEAD_DIM, HIDDEN), rope_frequencies(HEAD_DIM))
    ref = [v for row in ref for v in row]
    got = read_f32(pathlib.Path(out) / "layer0.f32", T * DIM)
    worst = max(abs(a - b) for a, b in zip(ref, got))
    scale = max(abs(v) for v in ref)
    print(f"layer 0: {T} tokens, max |difference| {worst:.3e}, largest |value| {scale:.3e}")
    if worst > LAYER_TOLERANCE * scale:
        print("layer 0 slice is outside the frozen tolerance", file=sys.stderr)
        return 1
    hidden = got[(T - 1) * DIM:]
    norm = load(directory, start, h, "norm.weight")
    n = rms(hidden, norm)
    head = load(directory, start, h, "tok_embeddings.weight", 0, READOUT_ROWS * DIM)
    ref = [_dot(head, r * DIM, DIM, n) for r in range(READOUT_ROWS)]
    got = read_f32(pathlib.Path(out) / "readout.f32", READOUT_ROWS)
    worst = max(abs(a - b) for a, b in zip(ref, got))
    scale = max(abs(v) for v in ref)
    print(f"readout rows 0..{READOUT_ROWS}: max |difference| {worst:.3e}, largest |value| {scale:.3e}")
    if worst > LAYER_TOLERANCE * scale:
        print("readout slice is outside the frozen tolerance", file=sys.stderr)
        return 1
    return 0


def main():
    a = sys.argv[1:]
    write = "--write" in a
    a = [v for v in a if v != "--write"]
    if a[:1] == ["pin"] and len(a) == 2:
        return pin(a[1], write)
    if a[:1] == ["tekken"] and len(a) == 2:
        return tekken(a[1], write)
    if a[:1] == ["tiny"] and len(a) == 1:
        return tiny(write)
    if a[:1] == ["slice-tokens"] and len(a) == 3 and not write:
        return slice_tokens(a[1], a[2])
    if a[:1] == ["slices"] and len(a) == 3 and not write:
        return slices(a[1], a[2])
    print(__doc__.strip().splitlines()[5].strip(), file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
