"""Chapter 164's checkpoint inventory: the tensor names, dtypes and shapes of
the pinned Nemotron 3.5 Lightning 30B-A3B checkpoint, written as a Coax
fixture, from the safetensors headers alone.

    python3 -I psionic/tests/nemotron_lightning_manifest.py --fetch DIRECTORY
    python3 -I psionic/tests/nemotron_lightning_manifest.py DIRECTORY [--write]

The checkpoint is 14 shards and 65.8 GB, more than this host's disk budget,
so --fetch downloads only config.json, model.safetensors.index.json and, by
HTTP range requests, the first 8 bytes and the JSON header of each shard
from https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16/resolve/a9904d24bcc1d289a1950fa9d2b978c47cf903b9/
(about 1 MB in all). The second form hashes config.json, the index and each
header against the pins, checks that each header's tensors tile its shard
exactly up to the pinned shard size, that the index names the same shard for
every tensor, and compares the fixture it would write with the committed
one; --write replaces it.

A header read by range cannot be checked against the shard's sha256 (that
needs the whole shard); its pin is the hash observed on first fetch. The
shard hashes are recorded below for the full download a later run makes.

Expert tensors are listed once per layer and kind: when a layer holds
experts 0 .. E-1 of one kind, all with the same dtype and shape, they become
one line named "experts.0..E-1." in place of the index. Otherwise every
tensor keeps its own line. It never imports or runs anything from the
download. Tooling only.
"""
import hashlib
import json
import pathlib
import re
import struct
import sys
import urllib.request

REPOSITORY = "nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16"
REVISION = "a9904d24bcc1d289a1950fa9d2b978c47cf903b9"
BASE = f"https://huggingface.co/{REPOSITORY}/resolve/{REVISION}/"
CONFIG_SHA256 = "a3827a0f5e311547b40943dc081e3ff2f8a277466e8c1a3df2291e8db8a7617c"
INDEX_SHA256 = "67f21da80ce245a3e24967f54eef3fa10e67a63eba16ef19a2d0569f06103f50"
# Shard sizes and sha256 (the Git LFS object ids at the pinned revision).
SHARDS = [
    (4991210024, "7a3e74d24969eac6657cb9b6422091fd062b7d30fd4bc90ecee6c3d7f692626a"),
    (4992601016, "a80c3846e959f6dda34aa031cbb668b766643691ac7e5c9f43f62e6fb1221059"),
    (4992601368, "996bf3a46668e14075cb82259390267c2b1f71446caeb3a5f1f9e0bd7a079ae7"),
    (4995692800, "cce1d79e7ff2e2b746b9d97166156aa1dcec796116675cc7fd1aa70a8d73ccb2"),
    (4980553832, "9338f59d3fd5f074288c71a9559452d5b5c581da00e9ccb9fe7aa2ce9b04d2ad"),
    (4989423536, "e947fa4097c68263ba3d097fceb32a300a22cebd9d909fb5198af7d18a4022fe"),
    (4992601488, "c7362630bf4256275ca58354ed8f31506cec67ffab30573e753703fd5ae48795"),
    (4992601520, "1b33a5a6b5505b25b57c90ddda19e9cd1598176468917df0c56116fa6f6072cb"),
    (4995692800, "fcc9b48caff427beaf1f87b78cf4ab42bc2a3c43b3c487bccb013ba22d7468b2"),
    (4992601520, "d1924fd54d1b77a15aff8df591a317e8a82a58e46d6f5742fa21e90e0be3cfa6"),
    (4995692800, "e9d0b83bcf04bd4d5758b94e6495a3f1595d121d82faffaa27ce6a884e2c8757"),
    (4995692816, "fce184f112cb0b97024cadc60f993f6f660e365d04e21c240fb3c75747bd3a01"),
    (3249723504, "f427d037d2a3e4d44323325aa38571e3551e8ad702f469c3dacc4e59ea2e50d1"),
    (2670685240, "64577b275ca4e7e5266eae0903674f7f46ec2a8cbf4f4f1a3207f80d503cd1d0"),
]
# sha256 of the 14 headers' JSON bytes, concatenated in shard order.
HEADERS_SHA256 = "b22bf2a2285a710b7ce7c311d0617928f8baef5de271f3bab29956fc3ec323ff"
FIXTURE = pathlib.Path(__file__).resolve().parent.parent / "fixtures" / "nemotron-lightning-manifest.coax"
DTYPE_BYTES = {"BF16": 2, "F32": 4}


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


def pattern(kinds: list) -> str:
    letters = {"mamba": "M", "attention": "*", "moe": "E"}
    return "".join(letters[k] for k in kinds)


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
            size = DTYPE_BYTES[t["dtype"]]
            for d in t["shape"]:
                size *= d
            first, last = t["data_offsets"]
            if last - first != size:
                print(f"refused: {name} holds {last - first} bytes, its shape needs {size}", file=sys.stderr)
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
    # Collapse each complete, uniform set of experts into one line.
    groups = {}
    for name in tensors:
        m = re.fullmatch(r"(.*\.experts\.)(\d+)(\..*)", name)
        if m:
            groups.setdefault((m.group(1), m.group(3)), []).append((int(m.group(2)), name))
    lines = {}
    folded = set()
    for (prefix, suffix), members in groups.items():
        ids = sorted(e for e, _ in members)
        kinds = {json.dumps(tensors[n]) for _, n in members}
        if ids == list(range(len(ids))) and len(kinds) == 1:
            dtype, shape = tensors[members[0][1]]
            lines[f"{prefix}0..{len(ids) - 1}{suffix}"] = (dtype, shape)
            folded.update(n for _, n in members)
    for name, value in tensors.items():
        if name not in folded:
            lines[name] = value
    fields = json.loads(config_bytes)
    keys = ["hidden_size", "vocab_size", "num_hidden_layers", "mamba_num_heads", "mamba_head_dim",
            "ssm_state_size", "n_groups", "conv_kernel", "num_attention_heads", "num_key_value_heads",
            "head_dim", "n_routed_experts", "num_experts_per_tok", "moe_intermediate_size",
            "moe_shared_expert_intermediate_size", "n_shared_experts", "num_nextn_predict_layers",
            "layer_norm_epsilon", "n_group", "topk_group", "norm_topk_prob", "routed_scaling_factor",
            "moe_latent_size", "mamba_hidden_act", "mlp_hidden_act", "use_conv_bias", "use_bias",
            "mamba_proj_bias", "attention_bias", "mlp_bias", "tie_word_embeddings"]
    settings = "\\n".join(f"{k} {json.dumps(fields[k])}".replace('"', "'") for k in keys)
    settings += f"\\nlayers_block_type '{pattern(fields['layers_block_type'])}'"
    settings += f"\\nmtp_layers_block_type '{pattern(fields['mtp_layers_block_type'])}'\\n"
    text = "\\n".join(f"{n} {lines[n][0]} {','.join(str(d) for d in lines[n][1])}" for n in sorted(lines)) + "\\n"
    total = sum(s for s, _ in SHARDS)
    out = (
        "// Chapter 164's checkpoint inventory, written by psionic/tests/nemotron_lightning_manifest.py\n"
        f"// from {REPOSITORY} at {REVISION}:\n"
        f"// the headers of its {len(SHARDS)} shards ({total} bytes in all; header bytes sha256\n"
        f"// {HEADERS_SHA256}), config.json sha256 {CONFIG_SHA256},\n"
        f"// model.safetensors.index.json sha256 {INDEX_SHA256}. No weights were downloaded.\n"
        "// One line a tensor, sorted by name: name, dtype, dimensions. A layer's experts\n"
        "// 0 .. E-1 of one kind, all alike, share one line named experts.0..E-1.\n"
        "// NL_PINNED_CONFIG copies the config fields the shape is built from (JSON\n"
        "// values, double quotes written as single quotes; the block type lists as\n"
        "// one letter a layer: M Mamba-2, * attention, E mixture of experts).\n"
        f'NL_PINNED_REVISION :: "{REVISION}";\n'
        f"NL_PINNED_BYTES :: {total};\n"
        f"NL_PINNED_TENSORS :: {len(tensors)};\n"
        f'NL_PINNED_CONFIG :: "{settings}";\n'
        f"NL_PINNED_LINES :: {len(lines)};\n"
        f'NL_PINNED_MANIFEST :: "{text}";\n'
    )
    if len(sys.argv) == 3:
        FIXTURE.write_text(out)
        print(f"wrote {FIXTURE} ({len(tensors)} tensors in {len(lines)} lines)")
        return 0
    if FIXTURE.read_text() != out:
        print(f"{FIXTURE} differs from the pinned checkpoint's headers", file=sys.stderr)
        return 1
    print(f"{len(tensors)} tensors in {len(lines)} lines: the fixture matches the pinned checkpoint's headers")
    return 0


if __name__ == "__main__":
    sys.exit(main())
