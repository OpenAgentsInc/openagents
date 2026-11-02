"""Chapter 85's checkpoint inventory: the tensor names, dtypes and shapes of
the pinned Mistral-Small-4-119B-2603 checkpoint (Hugging Face layout), written
as a Coax fixture, from the safetensors headers alone.

    python3 -I psionic/tests/mistral_small_manifest.py --fetch DIRECTORY
    python3 -I psionic/tests/mistral_small_manifest.py DIRECTORY [--write]

The checkpoint is 3 shards and 120.9 GB, more than this host's free disk, so
--fetch downloads only config.json, model.safetensors.index.json and, by HTTP
range requests, the first 8 bytes and the JSON header of each shard from
https://huggingface.co/mistralai/Mistral-Small-4-119B-2603/resolve/a11f36bebf709121056b1dbcc943d1c6afbe494d/
(about 4 MB in all). The second form hashes config.json, the index and the
headers against the pins, checks that each header's tensors tile its shard
exactly up to the pinned shard size, that the index names the same shard for
every tensor, and compares the fixture it would write with the committed one;
--write replaces it.

A header read by range cannot be checked against the shard's sha256 (that
needs the whole shard); its pin is the hash observed on first fetch. The
shard hashes (Git LFS object ids) are recorded below for a full download.

Layers are listed once per tensor kind: when layers 0 .. L-1 all hold a
tensor of the same dtype and shape under one name, they become one line named
"layers.0..L-1." in place of the index. A scalar's dimensions are written
"-". It never imports or runs anything from the download. Tooling only.
"""
import hashlib
import json
import pathlib
import re
import struct
import sys
import urllib.request

REPOSITORY = "mistralai/Mistral-Small-4-119B-2603"
REVISION = "a11f36bebf709121056b1dbcc943d1c6afbe494d"
BASE = f"https://huggingface.co/{REPOSITORY}/resolve/{REVISION}/"
CONFIG_SHA256 = "113ea221dcf0dc7adc2af52ad84933a5830b663b679a4753a78a98f757de8737"
INDEX_SHA256 = "30f9eae8bd2738cad0cd3bd4f7db8e3aac177f2b644bba0ff35a46a15efc8f39"
SHARDS = [
    (49078433296, "811a33f74a5160806bf59a5134f015a86386eea49d1b8d2f51c6509382cf5f03"),
    (49132710896, "50f010b6318591e49c41dda94926dfaa436f17219ec79f27289a72dd93e1effa"),
    (22711829744, "166755c02c0c0682302d460c0eadcf138758a022d9a578c7eb74f3251e770999"),
]
# sha256 of the 3 headers' JSON bytes, concatenated in shard order.
HEADERS_SHA256 = "376dd8d4c044f7c2332e1db30f780293e9496801e7eb59f67055eb6c20dd62c9"
FIXTURE = pathlib.Path(__file__).resolve().parent.parent / "fixtures" / "mistral-small-manifest.coax"
DTYPE_BYTES = {"BF16": 2, "F32": 4, "F8_E4M3": 1}
TEXT = "language_model."


def shard(i: int) -> str:
    return f"model-{i + 1:05d}-of-{len(SHARDS):05d}.safetensors"


def get(name: str, first: int | None = None, last: int | None = None) -> bytes:
    request = urllib.request.Request(BASE + name)
    if first is not None:
        request.add_header("Range", f"bytes={first}-{last}")
    with urllib.request.urlopen(request) as response:
        return response.read()


def fetch(directory: pathlib.Path) -> int:
    directory.mkdir(parents=True, exist_ok=True)
    for name in ("config.json", "model.safetensors.index.json"):
        (directory / name).write_bytes(get(name))
    for i in range(len(SHARDS)):
        n = struct.unpack("<Q", get(shard(i), 0, 7))[0]
        if not 2 <= n <= 1 << 20:
            print(f"refused: {shard(i)} header of {n} bytes", file=sys.stderr)
            return 1
        body = get(shard(i), 8, 7 + n)
        if len(body) != n:
            print(f"refused: {shard(i)} header came back {len(body)} bytes, not {n}", file=sys.stderr)
            return 1
        (directory / f"header-{i + 1:02d}.json").write_bytes(body)
    print(f"fetched config, index and {len(SHARDS)} headers into {directory}")
    return 0


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def size_of(dtype: str, shape: list) -> int:
    size = DTYPE_BYTES[dtype]
    for d in shape:
        size *= d
    return size


def main() -> int:
    if len(sys.argv) == 3 and sys.argv[1] == "--fetch":
        return fetch(pathlib.Path(sys.argv[2]))
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != "--write"):
        print(__doc__.strip().splitlines()[4].strip(), file=sys.stderr)
        return 2
    directory = pathlib.Path(sys.argv[1])
    config_bytes = (directory / "config.json").read_bytes()
    index_bytes = (directory / "model.safetensors.index.json").read_bytes()
    if sha(config_bytes) != CONFIG_SHA256 or sha(index_bytes) != INDEX_SHA256:
        print("refused: config.json or the index does not hash to its pin", file=sys.stderr)
        return 1
    headers = [(directory / f"header-{i + 1:02d}.json").read_bytes() for i in range(len(SHARDS))]
    if sha(b"".join(headers)) != HEADERS_SHA256:
        print("refused: the shard headers do not hash to their pin", file=sys.stderr)
        return 1
    weight_map = json.loads(index_bytes)["weight_map"]
    tensors = {}
    for i, raw in enumerate(headers):
        header = json.loads(raw)
        spans = []
        for name, t in header.items():
            if name == "__metadata__":
                continue
            if name in tensors or weight_map.get(name) != shard(i):
                print(f"refused: {name} repeated or not mapped to {shard(i)}", file=sys.stderr)
                return 1
            first, last = t["data_offsets"]
            if last - first != size_of(t["dtype"], t["shape"]):
                print(f"refused: {name} holds {last - first} bytes, its shape needs another size", file=sys.stderr)
                return 1
            spans.append((first, last))
            tensors[name] = (t["dtype"], t["shape"])
        spans.sort()
        at = 0
        for first, last in spans:
            if first != at:
                print(f"refused: {shard(i)} has a gap or overlap at byte {at}", file=sys.stderr)
                return 1
            at = last
        if 8 + len(raw) + at != SHARDS[i][0]:
            print(f"refused: {shard(i)} tensors end at {8 + len(raw) + at}, the pin says {SHARDS[i][0]}", file=sys.stderr)
            return 1
    if set(weight_map) != set(tensors):
        print("refused: the index and the headers name different tensors", file=sys.stderr)
        return 1
    # Fold each complete, uniform run of layers into one line.
    groups = {}
    for name in tensors:
        m = re.fullmatch(r"(.*\.layers\.)(\d+)(\..*)", name)
        if m:
            groups.setdefault((m.group(1), m.group(3)), []).append((int(m.group(2)), name))
    lines = {}
    folded = set()
    for (prefix, suffix), members in groups.items():
        ids = sorted(e for e, _ in members)
        kinds = {json.dumps(tensors[n]) for _, n in members}
        if ids == list(range(len(ids))) and len(kinds) == 1:
            lines[f"{prefix}0..{len(ids) - 1}{suffix}"] = tensors[members[0][1]]
            folded.update(n for _, n in members)
    for name, value in tensors.items():
        if name not in folded:
            lines[name] = value
    text_lines = sorted(n for n in lines if n.startswith(TEXT))
    other_lines = sorted(n for n in lines if not n.startswith(TEXT))
    other_bytes = sum(size_of(*tensors[n]) for n in tensors if not n.startswith(TEXT))
    other_count = sum(1 for n in tensors if not n.startswith(TEXT))
    header_bytes = sum(8 + len(raw) for raw in headers)
    t = json.loads(config_bytes)["text_config"]
    keys = ["hidden_size", "vocab_size", "num_hidden_layers", "num_attention_heads", "num_key_value_heads",
            "q_lora_rank", "kv_lora_rank", "qk_nope_head_dim", "qk_rope_head_dim", "v_head_dim",
            "n_routed_experts", "num_experts_per_tok", "moe_intermediate_size", "n_shared_experts",
            "max_position_embeddings", "first_k_dense_replace", "n_group", "topk_group", "norm_topk_prob",
            "routed_scaling_factor", "rms_norm_eps", "hidden_act", "rope_interleave", "tie_word_embeddings",
            "attention_bias", "mlp_bias"]
    settings = "\\n".join(f"{k} {json.dumps(t[k])}".replace('"', "'") for k in keys)
    rope = t["rope_parameters"]
    rkeys = ["rope_type", "rope_theta", "factor", "original_max_position_embeddings", "beta_fast", "beta_slow",
             "mscale", "mscale_all_dim", "llama_4_scaling_beta"]
    settings += "\\n" + "\\n".join(f"rope.{k} {json.dumps(rope[k])}".replace('"', "'") for k in rkeys)
    q = json.loads(config_bytes)["quantization_config"]
    settings += f"\\nquant_method '{q['quant_method']}'\\nactivation_scheme '{q['activation_scheme']}'\\nweight_block_size {json.dumps(q['weight_block_size'])}\\n"

    def line(n: str) -> str:
        dtype, shape = lines[n]
        return f"{n} {dtype} {','.join(str(d) for d in shape) or '-'}"

    text = "\\n".join(line(n) for n in text_lines) + "\\n"
    other = "\\n".join(line(n) for n in other_lines) + "\\n"
    total = sum(s for s, _ in SHARDS)
    out = (
        "// Chapter 85's checkpoint inventory, written by psionic/tests/mistral_small_manifest.py\n"
        f"// from {REPOSITORY} at {REVISION}:\n"
        f"// the headers of its {len(SHARDS)} shards ({total} bytes in all; header bytes sha256\n"
        f"// {HEADERS_SHA256}), config.json sha256 {CONFIG_SHA256},\n"
        f"// model.safetensors.index.json sha256 {INDEX_SHA256}. No weights were downloaded.\n"
        "// One line a tensor, sorted by name: name, dtype, dimensions (- for a scalar).\n"
        "// Layers 0 .. L-1 that all hold one tensor alike share one line named\n"
        "// layers.0..L-1. MS_PINNED_MANIFEST holds the text model's lines;\n"
        "// MS_PINNED_OTHER the vision tower's and the projector's, which this\n"
        "// chapter does not run. MS_PINNED_CONFIG copies the config fields the shape\n"
        "// is built from (JSON values, double quotes written as single quotes).\n"
        f'MS_PINNED_REVISION :: "{REVISION}";\n'
        f"MS_PINNED_BYTES :: {total};\n"
        f"MS_PINNED_HEADER_BYTES :: {header_bytes};\n"
        f"MS_PINNED_TENSORS :: {len(tensors)};\n"
        f'MS_PINNED_CONFIG :: "{settings}";\n'
        f"MS_PINNED_LINES :: {len(text_lines)};\n"
        f'MS_PINNED_MANIFEST :: "{text}";\n'
        f"MS_PINNED_OTHER_TENSORS :: {other_count};\n"
        f"MS_PINNED_OTHER_BYTES :: {other_bytes};\n"
        f'MS_PINNED_OTHER :: "{other}";\n'
    )
    if len(sys.argv) == 3:
        FIXTURE.write_text(out)
        print(f"wrote {FIXTURE} ({len(tensors)} tensors; {len(text_lines)} text lines)")
        return 0
    if FIXTURE.read_text() != out:
        print(f"{FIXTURE} differs from the pinned checkpoint's headers", file=sys.stderr)
        return 1
    print(f"{len(tensors)} tensors; {len(text_lines)} text lines: the fixture matches the pinned checkpoint's headers")
    return 0


if __name__ == "__main__":
    sys.exit(main())
