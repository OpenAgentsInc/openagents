"""Chapter 58's independent references for Llama-3.1-Minitron-4B-Width-Base.

Written from the published architecture (config.json: Llama 3.1 with
llama3 rotary scaling, rotate-half pairs as in Hugging Face's Llama layout,
an untied output head) and from tokenizer.json's merge list, not from Coax
and not from any downloaded code. Pure Python, float64. Tooling only.

    python3 -I psionic/tests/minitron_reference.py pin MODEL_DIRECTORY [--write]
        hash the pinned files, write MODEL_DIRECTORY/MANIFEST, and compare
        (or write) psionic/fixtures/minitron-manifest.coax from the headers
    python3 -I psionic/tests/minitron_reference.py tokens MODEL_DIRECTORY [--write]
        tokenize the fixture strings with tokenizer.json's merges; compare (or
        write) psionic/fixtures/minitron-tokens.coax
    python3 -I psionic/tests/minitron_reference.py tiny [--write]
        the synthetic two-layer model and the llama3 rotary frequencies;
        compare (or write) psionic/fixtures/minitron-tiny.coax
    python3 -I psionic/tests/minitron_reference.py slice-tokens MODEL_DIRECTORY OUT_DIRECTORY
        write OUT_DIRECTORY/tokens.txt, the prompt the slices run
    python3 -I psionic/tests/minitron_reference.py long-tokens MODEL_DIRECTORY OUT_DIRECTORY
        write OUT_DIRECTORY/tokens.txt, the 74-token passage the long slices run
    python3 -I psionic/tests/minitron_reference.py slices MODEL_DIRECTORY OUT_DIRECTORY
        recompute the original-weight slices the Coax harness wrote and compare

MODEL_DIRECTORY holds the files of
https://huggingface.co/nvidia/Llama-3.1-Minitron-4B-Width-Base at REVISION.
The script reads safetensors headers and the byte ranges of the tensors it
needs; it never imports or runs anything from the download.
"""
import hashlib
import json
import math
import pathlib
import re
import struct
import sys
from array import array

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
# Chapter 81's reference supplies Python-side helpers only: Coax string
# literals, and regex classes spelled out from unicodedata.
from ministral_reference import WHITE_SPACE, char_class, coax_string  # noqa: E402

REVISION = "5205ef7d36204947e3b973cb8b147a816ccd7e6a"
SOURCE = "https://huggingface.co/nvidia/Llama-3.1-Minitron-4B-Width-Base"
LICENSE = ("NVIDIA Open Model License Agreement (June 2024), "
           "https://developer.download.nvidia.com/licenses/nvidia-open-model-license-agreement-june-2024.pdf; "
           "NOTICE: Llama 3.1 is licensed under the Llama 3.1 Community License, Copyright Meta Platforms, Inc.")
SHARDS = ["model-00001-of-00002.safetensors", "model-00002-of-00002.safetensors"]
FILES = {
    SHARDS[0]: (4978354640, "585da9fc8ad7692fbf64a89148a424ab6c4907f1ed25881cf1e0b55070a9bff3"),
    SHARDS[1]: (4047172128, "5ec2029c584b3bef522e6f0d0027eff94c9b9db08d7fb543847b69abbe2dbe49"),
    "config.json": (906, None),
    "tokenizer.json": (9085657, None),
    "model.safetensors.index.json": (23949, None),
    "generation_config.json": (126, None),
    "tokenizer_config.json": (50500, None),
    "special_tokens_map.json": (301, None),
    "NOTICE": (571, None),
    "README.md": (5833, None),
}
# Filled in from the first hashing run and frozen; pin refuses any change.
PINNED = {
    "config.json": "41bb855b9e631ef74f73678d5eb9ef5dae5dd25d7a60d2ccab6553ed92ff6024",
    "tokenizer.json": "79e3e522635f3171300913bb421464a87de6222182a0570b9b2ccba2a964b2b4",
}
ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures"

# The published decoder (config.json).
DIM, LAYERS, HEADS, KV_HEADS, HEAD_DIM, HIDDEN, VOCAB = 3072, 32, 32, 8, 128, 9216, 128256
THETA, EPS = 500000.0, 1e-5
FACTOR, LOW_FACTOR, HIGH_FACTOR, ORIGINAL = 8.0, 1.0, 4.0, 8192
BOS, EOS = 128000, 128001

TOKEN_STRINGS = [
    "The capital of France is",
    "Hello, world!",
    "I'll say it: DON'T stop, they're HERE.",
    "  leading spaces\n\nand 1234567 numbers",
    "CamelCaseWord IPAddress x/y\r\n",
    "tabs\tand trailing   ",
    "a+b=c; // comment",
    "'resume' 'S",
    "",
    "caf\u00e9",
    "\u00c7a co\u00fbte 5 \u20ac",
    "\u041f\u0440\u0438\u0432\u0435\u0442, \u043c\u0438\u0440!",
    "\u65e5\u672c\u8a9e\u306e\u30c6\u30ad\u30b9\u30c8",
    "na\u00efve r\u00e9sum\u00e9",
    "e\u0301te\u0301 \u0301x",
    "\u0393\u03b5\u03b9\u03ac \u03c3\u03bf\u03c5",
    "\U0001f44d ok",
    "a\u00a0b \u00a0 c\u00a0\u0301",
    "\u01c5emal \u00bd\u00b2\u216b",
    "I'\u017f x '\u017fx",
    "\u0661\u0662\u0663\u0664\u0665 \u0666",
    "x\u2028y\u3000\u3000z",
]
# The prompt the original-weight slices run, and a passage that reaches past
# position 64 for the long slices.
SLICE_PROMPT = "The capital of France is"
LONG_PROMPT = ("The river rose in the night, and by morning the lower town was under water. People carried "
               "what they could up the hill to the church, where the priest had opened the doors and lit the "
               "stoves. Nobody knew how long the rain would last, so the baker began to bake bread for everyone "
               "who had come up from the river, and the children carried it")


def sha256(path):
    d = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 24), b""):
            d.update(chunk)
    return d.hexdigest()


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


def header(directory, shard):
    with open(pathlib.Path(directory) / shard, "rb") as f:
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
        want = digest or PINNED.get(name)
        if want is not None and got != want:
            print(f"refused: {name} sha256 {got} is not the pinned {want}", file=sys.stderr)
            return 1
        shas[name] = got
    config = json.loads((directory / "config.json").read_text())
    expect = {"hidden_size": DIM, "num_hidden_layers": LAYERS, "num_attention_heads": HEADS, "num_key_value_heads": KV_HEADS,
              "head_dim": HEAD_DIM, "intermediate_size": HIDDEN, "vocab_size": VOCAB, "rope_theta": THETA, "rms_norm_eps": EPS,
              "tie_word_embeddings": False, "hidden_act": "silu", "attention_bias": False, "mlp_bias": False,
              "bos_token_id": BOS, "eos_token_id": EOS, "torch_dtype": "bfloat16"}
    for k, v in expect.items():
        if config.get(k) != v:
            print(f"refused: config.json {k} is {config.get(k)!r}, expected {v!r}", file=sys.stderr)
            return 1
    if config["rope_scaling"] != {"factor": FACTOR, "high_freq_factor": HIGH_FACTOR, "low_freq_factor": LOW_FACTOR,
                                  "original_max_position_embeddings": ORIGINAL, "rope_type": "llama3"}:
        print("refused: config.json rope_scaling differs", file=sys.stderr)
        return 1
    lines = []
    for shard in SHARDS:
        _, h = header(directory, shard)
        for name in h:
            if name == "__metadata__":
                continue
            t = h[name]
            lines.append(f"{name} {t['dtype']} {','.join(str(d) for d in t['shape'])}")
    lines.sort()
    manifest = [f"source {SOURCE}", f"revision {REVISION}", f"license {LICENSE}",
                "note the .nemo archive was not downloaded; no downloaded code exists or runs"]
    manifest += [f"{shas[name]}  {name}" for name in FILES]
    (directory / "MANIFEST").write_text("\n".join(manifest) + "\n")
    text = "\\n".join(lines) + "\\n"
    out = (
        "// Chapter 58's checkpoint inventory, written by psionic/tests/minitron_reference.py pin\n"
        f"// from nvidia/Llama-3.1-Minitron-4B-Width-Base at {REVISION}\n"
        "// (NVIDIA Open Model License; Llama 3.1 Community License notice):\n"
        + "".join(f"// {name}, {FILES[name][0]} bytes, sha256 {shas[name]};\n" for name in (SHARDS + ["config.json", "tokenizer.json"]))
        + "// One line a tensor across both shards, sorted by name: name, dtype, dimensions. No weights.\n"
        f'LM_PINNED_REVISION :: "{REVISION}";\n'
        f"LM_PINNED_TENSORS :: {len(lines)};\n"
        f'LM_PINNED_MANIFEST :: "{text}";\n'
    )
    for name in PINNED:
        print(f"{name} sha256 {shas[name]}")
    return compare_or_write(FIXTURES / "minitron-manifest.coax", out, write, "the pinned checkpoint's headers")


# ---- tokens ----------------------------------------------------------------

# The Llama 3 split pattern over Unicode. Python's re has no \p{..} classes, so
# each class is spelled out as code point ranges from unicodedata (chapter 81's
# char_class), and \s and \S as Unicode White_Space. The contractions keep the
# pattern's (?i:...): Python's re folds case the way Unicode does, so 's also
# matches 'S and the long s, U+017F. The matching itself, alternatives in order
# and backtracking, is Python's regular-expression engine.
LLAMA3_PATTERN = (r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n]*"
                  r"|\s*[\r\n]+|\s+(?!\S)|\s+")
_PATTERN = []


def unicode_pattern():
    L = char_class(("Lu", "Ll", "Lt", "Lm", "Lo"))
    N = char_class(("Nd", "Nl", "No"))
    S = char_class((), WHITE_SPACE)
    return re.compile(
        r"(?i:'s|'t|'re|'ve|'m|'ll|'d)"
        rf"|[^\r\n{L}{N}]?[{L}]+"
        rf"|[{N}]{{1,3}}"
        rf"| ?[^{S}{L}{N}]+[\r\n]*"
        rf"|[{S}]*[\r\n]+"
        rf"|[{S}]+(?![^{S}])"
        rf"|[{S}]+"
    )


def pieces(text):
    if not _PATTERN:
        _PATTERN.append(unicode_pattern())
    out, at = [], 0
    while at < len(text):
        m = _PATTERN[0].match(text, at)
        assert m and m.end() > at
        out.append(text[at:m.end()].encode())
        at = m.end()
    return out


def byte_unicode():
    """GPT-2's printable alphabet: the byte-level string each byte becomes."""
    keep = list(range(33, 127)) + list(range(161, 173)) + list(range(174, 256))
    table, extra = {}, 0
    for b in range(256):
        if b in keep:
            table[b] = chr(b)
        else:
            table[b] = chr(256 + extra)
            extra += 1
    return table


def load_tokenizer(directory):
    data = json.loads((pathlib.Path(directory) / "tokenizer.json").read_text())
    model = data["model"]
    pre = data["pre_tokenizer"]["pretokenizers"]
    if model["type"] != "BPE" or not model["ignore_merges"] or pre[0]["pattern"]["Regex"] != LLAMA3_PATTERN:
        raise SystemExit("refused: tokenizer.json differs from the Llama 3 tokenizer this reference implements")
    if data["added_tokens"][0]["id"] != BOS or data["added_tokens"][0]["content"] != "<|begin_of_text|>":
        raise SystemExit("refused: tokenizer.json special tokens differ")
    vocab = model["vocab"]
    merges = {tuple(m.split(" ")): i for i, m in enumerate(model["merges"])}
    return vocab, merges


def merge_bpe(vocab, merges, piece):
    """Hugging Face's rule: map bytes to GPT-2 letters; a piece found whole in
    the vocabulary is one token (ignore_merges); otherwise repeatedly apply the
    earliest merge in the list among adjacent pairs, the leftmost on a tie."""
    table = byte_unicode()
    word = "".join(table[b] for b in piece)
    if word in vocab:
        return [vocab[word]]
    parts = list(word)
    while True:
        best, at = None, -1
        for i in range(len(parts) - 1):
            r = merges.get((parts[i], parts[i + 1]))
            if r is not None and (best is None or r < best):
                best, at = r, i
        if best is None:
            break
        parts[at:at + 2] = [parts[at] + parts[at + 1]]
    return [vocab[p] for p in parts]


def encode(vocab, merges, text):
    return [t for p in pieces(text) for t in merge_bpe(vocab, merges, p)]


def slice_tokens(directory, out, prompt=SLICE_PROMPT):
    vocab, merges = load_tokenizer(directory)
    tokens = [BOS] + encode(vocab, merges, prompt)
    (pathlib.Path(out) / "tokens.txt").write_text(" ".join(map(str, tokens)) + "\n")
    print(f"{len(tokens)} tokens: {tokens}")
    return 0


def tokens(directory, write):
    vocab, merges = load_tokenizer(directory)
    table = byte_unicode()
    # The fixture table is the vocabulary restricted to the fixture's pieces:
    # every entry whose bytes occur inside some piece, with its id as rank.
    subset, expected = {}, []
    for text in TOKEN_STRINGS:
        ids = []
        for p in pieces(text):
            for i in range(len(p)):
                for j in range(i + 1, len(p) + 1):
                    w = "".join(table[b] for b in p[i:j])
                    if w in vocab:
                        subset[p[i:j]] = vocab[w]
            ids += merge_bpe(vocab, merges, p)
        expected.append(ids)
    entries = sorted(subset.items())
    data, ends = [], [0]
    for piece, _ in entries:
        data += list(piece)
        ends.append(len(data))
    lines = [
        "// The Llama 3 vocabulary restricted to chapter 58's fixture strings, written by",
        f"// psionic/tests/minitron_reference.py tokens from tokenizer.json at {REVISION}.",
        "// Entry k is the bytes LM_TOKEN_DATA[LM_TOKEN_ENDS[k]..<LM_TOKEN_ENDS[k+1]] (some",
        "// are partial UTF-8 sequences) and its id, which is its rank. The expected ids",
        "// come from Hugging Face's merge-list rule over the full 280,147 merges, after",
        "// Python's re split the text with the Unicode pattern.",
        f"LM_TOKEN_ENTRIES :: {len(entries)};",
        f"LM_TOKEN_DATA_BYTES :: {len(data)};",
        f"LM_TOKEN_DATA :: [{len(data)}]u8.[" + ",".join(map(str, data)) + "];",
        f"LM_TOKEN_ENDS :: [{len(ends)}]int.[" + ",".join(map(str, ends)) + "];",
        f"LM_TOKEN_RANKS :: [{len(entries)}]int.[" + ",".join(str(r) for _, r in entries) + "];",
        f"LM_TOKEN_TEXTS :: [{len(TOKEN_STRINGS)}]string.[" + ",".join(coax_string(t) for t in TOKEN_STRINGS) + "];",
    ]
    flat, starts = [], [0]
    for ids in expected:
        flat += ids
        starts.append(len(flat))
    lines.append(f"LM_TOKEN_STARTS :: [{len(starts)}]int.[" + ",".join(map(str, starts)) + "];")
    lines.append(f"LM_TOKEN_IDS :: [{len(flat)}]int.[" + ",".join(map(str, flat)) + "];")
    return compare_or_write(FIXTURES / "minitron-tokens.coax", "\n".join(lines) + "\n", write, "the merge-list rule on the fixture strings")


# ---- the decoder -----------------------------------------------------------

def rope_frequencies(head_dim):
    """llama3 scaling: wavelengths below original/high_factor keep their
    frequency, above original/low_factor divide it by factor, and between
    blend by smooth = (original/wavelength - low)/(high - low)."""
    out = []
    for i in range(head_dim // 2):
        f = THETA ** (-2 * i / head_dim)
        wavelength = 2 * math.pi / f
        if wavelength < ORIGINAL / HIGH_FACTOR:
            out.append(f)
        elif wavelength > ORIGINAL / LOW_FACTOR:
            out.append(f / FACTOR)
        else:
            smooth = (ORIGINAL / wavelength - LOW_FACTOR) / (HIGH_FACTOR - LOW_FACTOR)
            out.append((1 - smooth) * f / FACTOR + smooth * f)
    return out


def rms(x, w):
    s = math.sqrt(sum(v * v for v in x) / len(x) + EPS)
    return [v / s * g for v, g in zip(x, w)]


def matvec(w, rows, cols, x):
    return [math.fsum(a * b for a, b in zip(w[r * cols:(r + 1) * cols], x)) for r in range(rows)]


def rotate(vec, heads, head_dim, position, freqs):
    """Hugging Face's rotate-half: element i pairs with element i + d/2."""
    out = list(vec)
    half = head_dim // 2
    for h in range(heads):
        for i in range(half):
            a, b = vec[h * head_dim + i], vec[h * head_dim + half + i]
            angle = position * freqs[i]
            c, s = math.cos(angle), math.sin(angle)
            out[h * head_dim + i] = a * c - b * s
            out[h * head_dim + half + i] = b * c + a * s
    return out


def layer(x_rows, w, shape, freqs):
    d, heads, kv, hd, hidden = shape
    qs, ks, vs = [], [], []
    for t, x in enumerate(x_rows):
        h = rms(x, w["attention_norm"])
        qs.append(rotate(matvec(w["wq"], heads * hd, d, h), heads, hd, t, freqs))
        ks.append(rotate(matvec(w["wk"], kv * hd, d, h), kv, hd, t, freqs))
        vs.append(matvec(w["wv"], kv * hd, d, h))
    out = []
    group = heads // kv
    for t in range(len(x_rows)):
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


# The synthetic model: chapter 81's tiny structure (width 8, 4 query heads, 2
# KV heads of 4, hidden 12, vocabulary 16, 2 layers, the same weight formula)
# with an untied head from formula 98, llama3 rotary frequencies and
# rotate-half pairs. Every weight is a multiple of 1/32, exact in BF16.
TINY = dict(d=8, heads=4, kv=2, hd=4, hidden=12, vocab=16, layers=2)
TINY_TOKENS = [1, 3, 7, 12, 4, 9, 0, 15, 2, 11]
# 80 tokens, so the cache reaches past position 64.
TINY_LONG = [(5 * i + 1) % 16 for i in range(80)]


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
            w[name] = [tiny_value(1 + l * 7 + j, i) for i in range(n)]
        w["attention_norm"] = [tiny_norm(100 + 2 * l, i) for i in range(d)]
        w["ffn_norm"] = [tiny_norm(101 + 2 * l, i) for i in range(d)]
        layers.append(w)
    embed = [tiny_value(0, i) for i in range(t["vocab"] * d)]
    head = [tiny_value(98, i) for i in range(t["vocab"] * d)]
    norm = [tiny_norm(99, i) for i in range(d)]
    return embed, layers, norm, head


def coax_floats(values):
    return ",".join(repr(float(v)) for v in values)


def tiny(write):
    t = TINY
    d = t["d"]
    embed, layers, norm, head = tiny_weights()
    # At head dimension 4 the two frequencies, 1 and 500000^(-1/2), fall in
    # the keep band and the blend band of llama3 scaling.
    freqs = rope_frequencies(t["hd"])
    x = [embed[tok * d:(tok + 1) * d] for tok in TINY_TOKENS]
    for w in layers:
        x = layer(x, w, (d, t["heads"], t["kv"], t["hd"], t["hidden"]), freqs)
    logits = []
    for row in x:
        h = rms(row, norm)
        logits += [math.fsum(a * b for a, b in zip(head[r * d:(r + 1) * d], h)) for r in range(t["vocab"])]
    x = [embed[tok * d:(tok + 1) * d] for tok in TINY_LONG]
    for w in layers:
        x = layer(x, w, (d, t["heads"], t["kv"], t["hd"], t["hidden"]), freqs)
    h = rms(x[-1], norm)
    long_logits = [math.fsum(a * b for a, b in zip(head[r * d:(r + 1) * d], h)) for r in range(t["vocab"])]
    big = rope_frequencies(HEAD_DIM)
    picks = [0, 1, 20, 28, 29, 32, 34, 35, 63]
    lines = [
        "// Chapter 58's float64 references, written by psionic/tests/minitron_reference.py tiny.",
        f"LM_TINY_TOKENS :: [{len(TINY_TOKENS)}]int.[" + ",".join(map(str, TINY_TOKENS)) + "];",
        f"LM_TINY_FREQS :: [{len(freqs)}]f64.[" + coax_floats(freqs) + "];",
        f"LM_TINY_LOGITS :: [{len(logits)}]f64.[" + coax_floats(logits) + "];",
        f"LM_TINY_LONG :: [{len(TINY_LONG)}]int.[" + ",".join(map(str, TINY_LONG)) + "];",
        f"LM_TINY_LONG_LOGITS :: [{len(long_logits)}]f64.[" + coax_floats(long_logits) + "];",
        f"LM_ROPE_PICKS :: [{len(picks)}]int.[" + ",".join(map(str, picks)) + "];",
        f"LM_ROPE_FREQS :: [{len(picks)}]f64.[" + coax_floats(big[i] for i in picks) + "];",
    ]
    return compare_or_write(FIXTURES / "minitron-tiny.coax", "\n".join(lines) + "\n", write, "the float64 reference")



# ---- original-weight slices -------------------------------------------------

class Checkpoint:
    def __init__(self, directory):
        self.directory = pathlib.Path(directory)
        self.where = {}
        for shard in SHARDS:
            start, h = header(directory, shard)
            for name, t in h.items():
                if name != "__metadata__":
                    self.where[name] = (shard, start, t)

    def load(self, name, first=0, count=None):
        shard, start, t = self.where[name]
        assert t["dtype"] == "BF16"
        n = 1
        for v in t["shape"]:
            n *= v
        count = n - first if count is None else count
        with open(self.directory / shard, "rb") as f:
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
    out = pathlib.Path(out)
    toks = [int(v) for v in (out / "tokens.txt").read_text().split()]
    c = Checkpoint(directory)
    T = len(toks)
    embed = []
    for tok in toks:
        embed += c.load("model.embed_tokens.weight", tok * DIM, DIM)
    if read_f32(out / "embed.f32", T * DIM) != embed:
        print("embedding slice differs", file=sys.stderr)
        return 1
    print(f"embed: {T} tokens, exact")
    p = "model.layers.0."
    w = {"wq": c.load(p + "self_attn.q_proj.weight"), "wk": c.load(p + "self_attn.k_proj.weight"),
         "wv": c.load(p + "self_attn.v_proj.weight"), "wo": c.load(p + "self_attn.o_proj.weight"),
         "w1": c.load(p + "mlp.gate_proj.weight"), "w2": c.load(p + "mlp.down_proj.weight"),
         "w3": c.load(p + "mlp.up_proj.weight"), "attention_norm": c.load(p + "input_layernorm.weight"),
         "ffn_norm": c.load(p + "post_attention_layernorm.weight")}
    ref = layer([embed[t * DIM:(t + 1) * DIM] for t in range(T)], w, (DIM, HEADS, KV_HEADS, HEAD_DIM, HIDDEN), rope_frequencies(HEAD_DIM))
    ref = [v for row in ref for v in row]
    got = read_f32(out / "layer0.f32", T * DIM)
    worst = max(abs(a - b) for a, b in zip(ref, got))
    scale = max(abs(v) for v in ref)
    print(f"layer 0: {T} tokens, max |difference| {worst:.3e}, largest |value| {scale:.3e}")
    if worst > LAYER_TOLERANCE * scale:
        print("layer 0 slice is outside the frozen tolerance", file=sys.stderr)
        return 1
    n = rms(got[(T - 1) * DIM:], c.load("model.norm.weight"))
    head = c.load("lm_head.weight", 0, READOUT_ROWS * DIM)
    ref = [math.fsum(a * b for a, b in zip(head[r * DIM:(r + 1) * DIM], n)) for r in range(READOUT_ROWS)]
    got = read_f32(out / "readout.f32", READOUT_ROWS)
    worst = max(abs(a - b) for a, b in zip(ref, got))
    scale = max(abs(v) for v in ref)
    print(f"readout rows 0..{READOUT_ROWS} of lm_head: max |difference| {worst:.3e}, largest |value| {scale:.3e}")
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
    if a[:1] == ["tokens"] and len(a) == 2:
        return tokens(a[1], write)
    if a[:1] == ["tiny"] and len(a) == 1:
        return tiny(write)
    if a[:1] == ["slice-tokens"] and len(a) == 3 and not write:
        return slice_tokens(a[1], a[2])
    if a[:1] == ["long-tokens"] and len(a) == 3 and not write:
        return slice_tokens(a[1], a[2], LONG_PROMPT)
    if a[:1] == ["slices"] and len(a) == 3 and not write:
        return slices(a[1], a[2])
    print(__doc__.strip().splitlines()[8].strip(), file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
