"""Chapter 163's tokenizer oracle: an independent encoder for the pinned
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
Nemotron Nano tokenizer.json, in plain Python.

    python3 -I psionic/tests/nemotron_tokens.py fixture MODEL_DIRECTORY
    python3 -I psionic/tests/nemotron_tokens.py check MODEL_DIRECTORY IDS_FILE
    python3 -I psionic/tests/nemotron_tokens.py texts OUT_FILE

The pre-tokenizer is the file's own Split pattern with its Unicode classes
rewritten for ASCII text and run by Python's re module; byte-level
characters come from GPT-2's bytes_to_unicode construction; merging is
GPT-2's loop (join every occurrence of the lowest-ranked pair per round).
None of it shares code with psionic/lib/nemotron-tokens.coax.

fixture writes psionic/fixtures/nemotron-tokens.coax: an excerpt of the
tokenizer.json (the same structure, holding every entry and merge the
fixture texts touch) and each text's ids over the full file. check compares
the ids the Coax encoder wrote over the full file (one line per text) with
this encoder's. Tooling only; nothing from the download is run.
"""
import hashlib
import json
import pathlib
import re
import sys

PIN = "623c34567aebb18582765289fbe23d901c62704d6518d71866e0e58db892b5b7"
TEXTS = [
    "The capital of France is",
    "Hello, world! HTMLParser camelCase ABc aB",
    "2024-11-02: pi is 3.14159, e is 2.71828",
    "  leading spaces\n\n  and\ttabs \n",
    "don't won't I'm they're WE'LL",
    "<s>special <|im_start|>user\nhi<|im_end|>\n",
    "a//b /* comment */ x/y\n/z",
    "https://example.com/path?q=1&r=two#frag",
    "trailing space ",
    "MixedCASEwords and UPPER lower\r\nCRLF",
    "\t\tindented()\n\treturn {x: [1, 2]};",
    "",
]


def bytes_to_unicode():
    bs = list(range(ord("!"), ord("~") + 1)) + list(range(ord("¡"), ord("¬") + 1)) + list(range(ord("®"), ord("ÿ") + 1))
    cs = bs[:]
    n = 0
    for b in range(256):
        if b not in bs:
            bs.append(b)
            cs.append(256 + n)
            n += 1
    return {b: chr(c) for b, c in zip(bs, cs)}


def load(directory):
    path = pathlib.Path(directory) / "tokenizer.json"
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != PIN:
        raise SystemExit("refused: tokenizer.json is not the pinned file")
    return json.loads(raw)


class Encoder:
    def __init__(self, t):
        pattern = [p for p in t["pre_tokenizer"]["pretokenizers"] if p["type"] == "Split"][0]["pattern"]["Regex"]
        for a, b in (("[^\\r\\n\\p{L}\\p{N}]", "[^\\r\\nA-Za-z0-9]"), ("[\\p{Lu}\\p{Lt}\\p{Lm}\\p{Lo}\\p{M}]", "[A-Z]"),
                     ("[\\p{Ll}\\p{Lm}\\p{Lo}\\p{M}]", "[a-z]"), ("[^\\s\\p{L}\\p{N}]", "[^\\sA-Za-z0-9]"), ("\\p{N}", "[0-9]")):
            pattern = pattern.replace(a, b)
        if "\\p" in pattern:
            raise SystemExit("refused: a Unicode class was not rewritten")
        self.split = re.compile(pattern, re.ASCII)
        self.vocab = t["model"]["vocab"]
        self.ranks = {tuple(m): r for r, m in enumerate(t["model"]["merges"])}
        self.added = sorted(((a["content"], a["id"]) for a in t["added_tokens"]), key=lambda x: -len(x[0]))
        self.byte = bytes_to_unicode()
        self.seen = set()
        self.used = set()

    def bpe(self, word):
        self.used.add(word)
        if word in self.vocab:
            return [self.vocab[word]]
        parts = list(word)
        while True:
            pairs = {(parts[i], parts[i + 1]) for i in range(len(parts) - 1)}
            self.seen |= pairs
            ranked = [(self.ranks[p], p) for p in pairs if p in self.ranks]
            if not ranked:
                break
            a, b = min(ranked)[1]
            out, i = [], 0
            while i < len(parts):
                if i + 1 < len(parts) and parts[i] == a and parts[i + 1] == b:
                    out.append(a + b)
                    i += 2
                else:
                    out.append(parts[i])
                    i += 1
            parts = out
        self.used |= set(parts)
        return [self.vocab[p] for p in parts]

    def segment(self, text):
        ids = []
        for piece in self.split.findall(text):
            ids += self.bpe("".join(self.byte[b] for b in piece.encode()))
        return ids

    def encode(self, text):
        ids, start, i = [], 0, 0
        while i < len(text):
            hit = next(((c, n) for c, n in self.added if text.startswith(c, i)), None)
            if hit:
                ids += self.segment(text[start:i]) + [hit[1]]
                i += len(hit[0])
                start = i
            else:
                i += 1
        return ids + self.segment(text[start:])


def coax_string(s):
    out = []
    for ch in s:
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\t":
            out.append("\\t")
        elif ch == "\r":
            out.append("\\r")
        else:
            out.append(ch)
    return '"' + "".join(out) + '"'


def fixture(directory):
    t = load(directory)
    enc = Encoder(t)
    ids = [enc.encode(text) for text in TEXTS]
    used_added = {c for c, _ in enc.added if any(c in text for text in TEXTS)}
    byte = bytes_to_unicode()
    keep = set(enc.used) | set(byte.values()) | {a + b for a, b in enc.seen} | {a for a, _ in enc.seen} | {b for _, b in enc.seen}
    keep |= used_added
    vocab = {k: v for k, v in t["model"]["vocab"].items() if k in keep}
    merges = [m for m in t["model"]["merges"] if tuple(m) in enc.seen]
    excerpt = {
        "version": t["version"],
        "added_tokens": [a for a in t["added_tokens"] if a["content"] in used_added],
        "normalizer": t["normalizer"],
        "pre_tokenizer": t["pre_tokenizer"],
        "post_processor": t["post_processor"],
        "decoder": t["decoder"],
        "model": {k: (vocab if k == "vocab" else merges if k == "merges" else v) for k, v in t["model"].items()},
    }
    text = json.dumps(excerpt, ensure_ascii=False, indent=1).encode()
    # The excerpt must give the same ids as the whole file.
    small = Encoder(json.loads(text))
    if [small.encode(x) for x in TEXTS] != ids:
        raise SystemExit("refused: the excerpt does not reproduce the full file's ids")
    starts = [0]
    for i in ids:
        starts.append(starts[-1] + len(i))
    flat = [v for i in ids for v in i]
    lines = [
        "// Chapter 163's tokenizer fixture, written by psionic/tests/nemotron_tokens.py",
        f"// from the pinned tokenizer.json (sha256 {PIN}).",
        f"// NT_EXCERPT is that file cut to the {len(vocab)} entries and {len(merges)} merges the fixture",
        "// texts touch, in the same structure; NT_IDS are each text's ids over the full",
        "// 131,072-entry, 269,443-merge file, from the Python encoder.",
        f"NT_EXCERPT :: [{len(text)}]u8.[{','.join(map(str, text))}];",
        f"NT_TEXTS :: [{len(TEXTS)}]string.[{','.join(coax_string(x) for x in TEXTS)}];",
        f"NT_STARTS :: [{len(starts)}]int.[{','.join(map(str, starts))}];",
        f"NT_IDS :: [{len(flat)}]int.[{','.join(map(str, flat))}];",
        "",
    ]
    out = pathlib.Path(__file__).resolve().parents[1] / "fixtures" / "nemotron-tokens.coax"
    out.write_text("\n".join(lines))
    print(f"wrote {out}: {len(vocab)} entries, {len(merges)} merges, {len(text)} bytes, {len(flat)} ids")
    for x, i in zip(TEXTS, ids):
        print(repr(x), i)


def check(directory, path):
    enc = Encoder(load(directory))
    got = [list(map(int, line.split())) for line in pathlib.Path(path).read_text().splitlines()]
    want = [enc.encode(x) for x in TEXTS]
    ok = got == want
    for x, g, w in zip(TEXTS, got, want):
        print(f"{'same' if g == w else 'DIFFERENT'} {len(w):3d} ids  {x!r}")
    print(f"tokenizer over the full file: {sum(map(len, want))} ids, Coax and Python {'agree' if ok else 'DISAGREE'}")
    return 0 if ok else 1


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "fixture":
        fixture(sys.argv[2])
        return 0
    if len(sys.argv) == 3 and sys.argv[1] == "texts":
        pathlib.Path(sys.argv[2]).write_bytes(b"".join(x.encode() + b"\0" for x in TEXTS))
        return 0
    if len(sys.argv) == 4 and sys.argv[1] == "check":
        return check(sys.argv[2], sys.argv[3])
    print(__doc__.strip().splitlines()[3].strip(), file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
