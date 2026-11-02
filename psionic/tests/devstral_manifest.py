"""Chapter 83's pins for Devstral Small 2: the text decoder's tensor
inventory, read from the safetensors headers alone, and Tekken's ids for the
chapter's coding-agent conversation, from the full tekken.json.

    python3 -I psionic/tests/devstral_manifest.py --fetch DIRECTORY
    python3 -I psionic/tests/devstral_manifest.py DIRECTORY [--write]

The consolidated FP8 checkpoint is two shards and 25.8 GB, so --fetch
downloads only params.json, config.json, the consolidated index,
chat_template.jinja, tekken.json (16.8 MB) and, by HTTP range requests, the
first 8 bytes and the JSON header of each shard from
https://huggingface.co/mistralai/Devstral-Small-2-24B-Instruct-2512/resolve/55c5b41e98c2dbd21b0c8afffc540dcfc9eb5128/
The second form hashes every file against the pins, checks that each
header's tensors tile its shard exactly up to the pinned shard size and that
the index names the same shard for every tensor, then compares the two
fixtures it would write with the committed ones; --write replaces them.

A header read by range cannot be checked against the shard's sha256 (that
needs the whole shard); its pin is the hash observed on first fetch. The
shard hashes are recorded below for the full download a later run makes.

The conversation's expected text is json.dumps with its default separators,
which is what the pinned chat template's tojson prints under Hugging Face's
renderer; its ids are tiktoken's rule over the full Tekken table, through
chapter 81's psionic/tests/ministral_reference.py. Nothing from the download
is imported or run. Tooling only.
"""
import hashlib
import json
import pathlib
import struct
import sys
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import ministral_reference as mr  # noqa: E402  (chapter 81's Tekken helpers)

REPOSITORY = "mistralai/Devstral-Small-2-24B-Instruct-2512"
REVISION = "55c5b41e98c2dbd21b0c8afffc540dcfc9eb5128"
BASE = f"https://huggingface.co/{REPOSITORY}/resolve/{REVISION}/"
PINS = {
    "params.json": "1618da0d6f897e074ff7ec4b1047b5d2e8a1a4aa1fab41e26fc88f3b5b94d2aa",
    "config.json": "8d570ae877badd9ff5442afdc726385aba07f5757ba875a7491bde9549457e72",
    "consolidated.safetensors.index.json": "ae098214199fb7fe4948768c0a640f0cea32d1effb827c09b0ae1314b02ef714",
    "chat_template.jinja": "a0c0ed8a45c2c75c85fd756316a10419b90eb4f3dbe64d00f5c0b8ad4de5209a",
    "tekken.json": "600bb27946565481ecf51ba8aee252e49b9a68507866080ac9c30185bb312843",
}
# Shard sizes and sha256 (the Git LFS object ids at the pinned revision).
SHARDS = [
    (19839869982, "dfa96c3ccb824ac308eeeaa86fd1ce01aca4e3311e1aaa27a498ec3b7302e165"),
    (5953165906, "b783163b5ee6fb9595fde29d6072e81be8fcc24ea576d09ecc3dc7611ababb97"),
]
# sha256 of the two headers' JSON bytes, concatenated in shard order.
HEADERS_SHA256 = "a48c14f62d7bfe76b94f502ab37a157f94222b79ed595fc1cd26ea88f7b5e54e"
FIXTURES = HERE.parent / "fixtures"
DTYPE_BYTES = {"BF16": 2, "F8_E4M3": 1, "F32": 4}
TEXT_PREFIXES = ("layers.", "norm.", "output.", "tok_embeddings.")

# The chapter's conversation. The tools, in the order the agent offers them.
SYSTEM = "You write Stack programs. Call run to try one, then submit your answer."
CODE = {"type": "string", "pattern": "^[x0-9*+-]{1,11}$"}
TOOLS = [
    {"type": "function", "function": {"name": "run", "description": "Run a Stack program on one input.",
        "parameters": {"type": "object", "properties": {"code": CODE, "x": {"type": "integer", "minimum": 0, "maximum": 999}},
                       "required": ["code", "x"]}}},
    {"type": "function", "function": {"name": "submit", "description": "Submit a Stack program for the hidden tests.",
        "parameters": {"type": "object", "properties": {"code": CODE}, "required": ["code"]}}},
]
USER = ("Write f(x) = 2x + 1 in Stack. x pushes the input, a digit pushes itself, "
        "and + - * pop two values and push the result. Example: f(3) = 7.")
CALLS = [("run", {"code": "x2*1+", "x": 3}, "7"), ("submit", {"code": "x2*1+"}, None)]


def shard(i):
    return f"consolidated-{i + 1:05d}-of-{len(SHARDS):05d}.safetensors"


def get(name, first=None, last=None):
    request = urllib.request.Request(BASE + name)
    if first is not None:
        request.add_header("Range", f"bytes={first}-{last}")
    with urllib.request.urlopen(request) as response:
        return response.read()


def fetch(directory):
    directory.mkdir(parents=True, exist_ok=True)
    for name in PINS:
        (directory / name).write_bytes(get(name))
    for i in range(len(SHARDS)):
        n = struct.unpack("<Q", get(shard(i), 0, 7))[0]
        if not 2 <= n <= 1 << 21:
            print(f"refused: {shard(i)} header of {n} bytes", file=sys.stderr)
            return 1
        body = get(shard(i), 8, 7 + n)
        if len(body) != n:
            print(f"refused: {shard(i)} header came back {len(body)} bytes, not {n}", file=sys.stderr)
            return 1
        (directory / f"header-{i + 1:02d}.json").write_bytes(body)
    print(f"fetched {len(PINS)} files and {len(SHARDS)} headers into {directory}")
    return 0


def sha(data):
    return hashlib.sha256(data).hexdigest()


def conversation():
    """The rendered text, split into control tokens and text segments."""
    parts = [("<s>", 1), ("[SYSTEM_PROMPT]", 17), SYSTEM, ("[/SYSTEM_PROMPT]", 18),
             ("[AVAILABLE_TOOLS]", 5), json.dumps(TOOLS), ("[/AVAILABLE_TOOLS]", 6),
             ("[INST]", 3), USER, ("[/INST]", 4)]
    for name, arguments, result in CALLS:
        parts += [("[TOOL_CALLS]", 9), name, ("[ARGS]", 32), json.dumps(arguments), ("</s>", 2)]
        if result is not None:
            parts += [("[TOOL_RESULTS]", 7), result, ("[/TOOL_RESULTS]", 8)]
    return parts


def tekken_fixture(directory):
    ranks = mr.load_tekken(directory)
    parts = conversation()
    texts = [p for p in parts if isinstance(p, str)]
    subset, ids, rendered = {}, [], ""
    for p in parts:
        if isinstance(p, tuple):
            ids.append(p[1])
            rendered += p[0]
            continue
        rendered += p
        for piece in mr.pieces(p):
            for i in range(len(piece)):
                for j in range(i + 1, len(piece) + 1):
                    if piece[i:j] in ranks:
                        subset[piece[i:j]] = ranks[piece[i:j]]
            ids += [mr.SPECIALS + r for r in mr.bpe(ranks, piece)]
    entries = sorted(subset.items())
    lines = [
        "// Tekken v13 restricted to chapter 83's conversation, written by",
        f"// psionic/tests/devstral_manifest.py from tekken.json at {REVISION}",
        f"// (sha256 {PINS['tekken.json']}, the same file chapter 81 pins). Each",
        "// entry is a byte string and its original rank; token ids are rank + 1000.",
        "// DV_CHAT_TEXT is the conversation as the pinned chat template renders it",
        "// (json.dumps defaults for tools and arguments); DV_CHAT_IDS are its ids,",
        "// tiktoken's rule run in Python over the full 130,072-entry table, with the",
        "// control tokens <s> 1, [INST] 3, [/INST] 4, [AVAILABLE_TOOLS] 5,",
        "// [/AVAILABLE_TOOLS] 6, [TOOL_RESULTS] 7, [/TOOL_RESULTS] 8, [TOOL_CALLS] 9,",
        "// [SYSTEM_PROMPT] 17, [/SYSTEM_PROMPT] 18, [ARGS] 32 and </s> 2.",
        f"DV_TEKKEN_BYTES :: [{len(entries)}]string.[" + ",".join(mr.coax_string(b.decode()) for b, _ in entries) + "];",
        f"DV_TEKKEN_RANKS :: [{len(entries)}]int.[" + ",".join(str(r) for _, r in entries) + "];",
        f"DV_TOOLS_JSON :: {mr.coax_string(texts[1])};",
        f"DV_CHAT_TEXT :: {mr.coax_string(rendered)};",
        f"DV_CHAT_IDS :: [{len(ids)}]int.[" + ",".join(map(str, ids)) + "];",
    ]
    return "\n".join(lines) + "\n"


def manifest_fixture(directory, headers, params):
    weight_map = json.loads((directory / "consolidated.safetensors.index.json").read_bytes())["weight_map"]
    tensors = {}
    for i, raw in enumerate(headers):
        header = json.loads(raw)
        spans = []
        for name, t in header.items():
            if name == "__metadata__":
                continue
            if name in tensors or weight_map.get(name) != shard(i):
                raise SystemExit(f"refused: {name} repeated or not mapped to {shard(i)}")
            size = DTYPE_BYTES[t["dtype"]]
            for d in t["shape"]:
                size *= d
            first, last = t["data_offsets"]
            if last - first != size:
                raise SystemExit(f"refused: {name} holds {last - first} bytes, its shape needs {size}")
            spans.append((first, last))
            tensors[name] = (t["dtype"], t["shape"], size)
        spans.sort()
        at = 0
        for first, last in spans:
            if first != at:
                raise SystemExit(f"refused: {shard(i)} has a gap or overlap at byte {at}")
            at = last
        if 8 + len(raw) + at != SHARDS[i][0]:
            raise SystemExit(f"refused: {shard(i)} tensors end at {8 + len(raw) + at}, the pin says {SHARDS[i][0]}")
    if set(weight_map) != set(tensors):
        raise SystemExit("refused: the index and the headers name different tensors")
    text = sorted(n for n in tensors if n.startswith(TEXT_PREFIXES))
    other = [n for n in tensors if not n.startswith(TEXT_PREFIXES)]
    text_bytes = sum(tensors[n][2] for n in text)
    other_bytes = sum(tensors[n][2] for n in other)
    keys = ["dim", "n_layers", "head_dim", "hidden_dim", "n_heads", "n_kv_heads", "vocab_size", "tied_embeddings",
            "rope_theta", "norm_eps", "max_position_embeddings"]
    settings = "\\n".join(f"{k} {json.dumps(params[k])}" for k in keys)
    settings += f"\\nqformat_weight {params['quantization']['qformat_weight']}\\nqscheme_act {params['quantization']['qscheme_act']}\\n"
    lines = "\\n".join(f"{n} {tensors[n][0]} {','.join(str(d) for d in tensors[n][1]) or 'scalar'}" for n in text) + "\\n"
    total = sum(s for s, _ in SHARDS)
    return (
        "// Chapter 83's checkpoint inventory, written by psionic/tests/devstral_manifest.py\n"
        f"// from {REPOSITORY} at {REVISION} (Apache-2.0):\n"
        f"// the headers of consolidated-0000{{1,2}}-of-00002.safetensors ({total} bytes in all;\n"
        f"// header bytes sha256 {sha(b''.join(headers))}),\n"
        f"// params.json sha256 {PINS['params.json']}. No weights were downloaded.\n"
        "// One line a text-decoder tensor, sorted by name: name, dtype, dimensions\n"
        "// (scalar for a 0-dimensional tensor). The image path's tensors are counted,\n"
        "// not listed. DV_PINNED_PARAMS copies the params.json fields the shape is\n"
        "// built from.\n"
        f'DV_PINNED_REVISION :: "{REVISION}";\n'
        f"DV_PINNED_BYTES :: {total};\n"
        f"DV_PINNED_TENSORS :: {len(tensors)};\n"
        f"DV_PINNED_TEXT_BYTES :: {text_bytes};\n"
        f"DV_PINNED_IMAGE_TENSORS :: {len(other)};\n"
        f"DV_PINNED_IMAGE_BYTES :: {other_bytes};\n"
        f'DV_PINNED_PARAMS :: "{settings}";\n'
        f"DV_PINNED_LINES :: {len(text)};\n"
        f'DV_PINNED_MANIFEST :: "{lines}";\n'
    )


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "--fetch":
        return fetch(pathlib.Path(sys.argv[2]))
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != "--write"):
        print(__doc__.strip().splitlines()[4].strip(), file=sys.stderr)
        return 2
    directory = pathlib.Path(sys.argv[1])
    for name, pin in PINS.items():
        if sha((directory / name).read_bytes()) != pin:
            print(f"refused: {name} does not hash to its pin", file=sys.stderr)
            return 1
    headers = [(directory / f"header-{i + 1:02d}.json").read_bytes() for i in range(len(SHARDS))]
    if sha(b"".join(headers)) != HEADERS_SHA256:
        print("refused: the shard headers do not hash to their pin", file=sys.stderr)
        return 1
    params = json.loads((directory / "params.json").read_bytes())
    write = len(sys.argv) == 3
    status = mr.compare_or_write(FIXTURES / "devstral-manifest.coax", manifest_fixture(directory, headers, params),
                                 write, "the pinned checkpoint's headers")
    status |= mr.compare_or_write(FIXTURES / "devstral-tekken.coax", tekken_fixture(directory), write,
                                  "Tekken on the chapter's conversation")
    return status


if __name__ == "__main__":
    sys.exit(main())
