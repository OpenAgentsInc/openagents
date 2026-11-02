#!/usr/bin/env python3
# Firm reason: 1 (checkpoint inventory: reads safetensors headers by HTTP range; tooling only)
"""Chapter 100's checkpoint inventory: the tensor names, dtypes and shapes of
the pinned DeepSeek V4.1 Flash checkpoint, written as a Coax fixture, from the
safetensors headers alone.

    python3 -I psionic/tests/deepseek_v41_manifest.py --fetch DIRECTORY
    python3 -I psionic/tests/deepseek_v41_manifest.py DIRECTORY [--write]

The checkpoint is 48 shards and 510.3 GB, far more than this host's disk, so
--fetch downloads only config.json, model.safetensors.index.json and, by HTTP
range requests, the first 8 bytes and the JSON header of each shard from
https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/resolve/1bae0e1088993b9b82de08f59de5be5a34e42170/
(about 9 MB in all). The second form hashes config.json, the index and the
headers against the pins, checks that each header's tensors tile its shard
exactly up to the pinned shard size, that the index names the same shard for
every tensor and states the same total, and compares the fixture it would
write with the committed one; --write replaces it.

A header read by range cannot be checked against the shard's sha256 (that needs
the whole shard); its pin is the hash observed on first fetch. The shard sizes
and sha256 below are the Git LFS objects at the pinned revision, as the Hub's
tree listing reported them on 2026-11-02 (license tag mit, not gated; the
redistribution question stays with the owner).

Per-expert tensors are listed once per layer and kind: when a layer holds
experts 0 .. E-1 of one kind, all with the same dtype and shape, they become
one row named "...experts.*.w1.weight". Tensors that differ only in their layer
number and share dtype and shape become one row with a layer set. It never
imports or runs anything from the download. Tooling only.
"""
import hashlib
import json
import pathlib
import re
import struct
import sys
import urllib.request

REPOSITORY = "deepseek-ai/DeepSeek-V4.1-Flash-1017"
REVISION = "1bae0e1088993b9b82de08f59de5be5a34e42170"
BASE = f"https://huggingface.co/{REPOSITORY}/resolve/{REVISION}/"
CONFIG_SHA256 = "8be45ce0476004a3f529fd896115a4a2e800a129ad2d3ec05b16050f52e21879"
INDEX_SHA256 = "74b0686a3d2891980d5e303251b075a3bccae2c2ff650747db2620a649b98fa8"
HEADERS_SHA256 = "0a1ab5659a937fe911a7b10154830e03b4e2ee14c0c3c488f88d744ca106922e"
SHARDS = [
    (970533624, "13bd121075f0b68cb7e8e8021a1011d42a81a83f557a2602fd9c07d52fddb0ec"),
    (1323858272, "43616eebd88dd4f1f1b642772356fc160478d1d9d04edd240431e7d065d2eda7"),
    (7389759032, "41611994fc9b27829b757d72257570d65a976fe65b8c6df8641adcd139d65ded"),
    (7389759032, "e8e901b3cb1ab46f32729c7eaab37bd774a09094f0a02c51a60e883f779f2c39"),
    (7405953784, "c5df71c02d6a22bcb20a0dbbcda9c6b65aa104f44415102a9a6a03bee92bb9de"),
    (7389759032, "a5d42afde2aee33619fb7a674b4c737c38d1f1d47a636a26f3edfb3b54164167"),
    (7389759032, "24856942add8f73f5e1fbaea1e9c9b6ff597fd1b4aa4651d789833dd3a5a2ab9"),
    (7389759032, "7bf5f7afb5d2bacad49feffb049989fb365d6dbf96986e76265bd7a7a3469564"),
    (7389759032, "e6ae47e9daca138fda8c04f833c8328eb21671c37d1c8e2c3f7a82aac26649f6"),
    (7389759032, "835e9a8b0a28cc571cb1cf9e6014b3d5eb84ac41990a8aa4519e1ac435d7e23c"),
    (7405953784, "5df968edefe59033e451ff8512bef57da2cf97b57b4e7a1380118489dd02915b"),
    (7389759032, "4f97bf292f58178ecdf593b55252a401d6e9102f18625a4fccc52b8ee57e7be0"),
    (7389761368, "1b8567854887396840eb5392ca763101308064a142cb36156159096e581659eb"),
    (7389761368, "e5ba6d7e4380728ad70b8208676075a898f80a64c3618060f1b81e4218a159d1"),
    (7389761368, "f4b29d936024e68f44fb14d0ffffb15b42b27be04b4a0296acdae695afa94c30"),
    (7389761368, "3aa67228351c80e708dab4ac7f6e56b19000649cce2cd655def3168a3de4115c"),
    (7405956128, "b704eec7b7aa1bad6c9d57fa7f61f2db28862e8e5fc2468fd9d872689c778c7d"),
    (7389761368, "1405cf03376739f46e72e6cdd4d404969f1b94c717bb01782a300e2fbbd17e29"),
    (7389761368, "a84beaded1913e8d6119ac111f25040c8ec42e59f200dbe04c3a991cac512669"),
    (7389761368, "038914f79cf5499da90f0bfb8e1b7761cc37d5f4dd132e2431a6d02c9fb9fdfe"),
    (7389761368, "2ba49f59c8cec144e41370f9f820f268899d0b5cd0501e881908ca65bccd2dc2"),
    (7389761368, "52f12cceca5e2fdd0191dfbd33b808b6c7ce844cf9351c119413c5ba3bcd28c8"),
    (7400713088, "d6e47e8b7147c64eb6619732694dda811e1a9f8af1894977919af9522724ca31"),
    (7389761368, "9d611d65374d37f5e02fdae14e680cb69bf55af8a8478802ac465b97c65be22b"),
    (7389761368, "8db532a7cbb23cd4b5aa775636ca13af95b758501290fb935074348f660a29cd"),
    (7389761368, "2d6d7261937d2a98c7398a55ed48dbf129d9b4f188aa9148e46ea9f720255115"),
    (7395337384, "8e76de6a5cdc7ac0923526d1c1a2e6459c55bbfa692793b29cdfe07b4f9fbc52"),
    (7389761368, "3c0e5ad6a4e8e811c1a55fd8f5b59c9fa329b241c671997c28fc7a01944d5a77"),
    (7389761368, "392a950bcb062b89f683abc52d49a17e92c66f80cd1cb6fb92b71091da75319b"),
    (7389761368, "0b44d1364e578fc26dc0cc86564e49a92871a51063fb7f2f3b51cb020de44ab0"),
    (7395337384, "d3230ffc32e11c5a34505c5acaeaff642aed5c770b297695e1f7a94071d361ac"),
    (7389761368, "96d284016ac885415859346d971c0de43bd9e7947337edfb53c34462866ef1d7"),
    (7389761368, "313953b3b6aaa6d4c60bbcfc177dda5d8d1380c9fe72d82560ba078f55278a8d"),
    (7389761368, "98eb27655774796bb3feadb35ec64a9849104b88c4c6ae7cabf6fc1ee317a056"),
    (7395337384, "4a321a28c048389039e4595641fe0b1f130428c61fc8f655545afe0083d37cd9"),
    (7389761368, "97c7910a8a14002dd941f45f6ddb2bd335eaf6fa05f4a76b2ff9285d6557b65d"),
    (7389761368, "d37ee013b805344619536e299c73adf92df03744aa0a242f2c5465590015e5be"),
    (7389761368, "7713bf7a78319eb1c6214b312e0a2da8cac88d77dce851d50f1113d78f8975a4"),
    (7395337384, "658a77d47d217995b9b78612e69ee0d0e33f6c2240f4ec629fecdca3ecb03f4c"),
    (7389761368, "6a9dbcc8d5894ba614b5e962764f45f6d0df7f14f1287b17f173d1f2cd762186"),
    (7389761368, "458308f08ed29b34850bd6b9d10bf769620729d3725c77f7bc0c1d14e01b47cc"),
    (7389761368, "3d283a4ed6eb2d5013bacc97c3500361926a3261957943434513304df52adfd7"),
    (1323837624, "4ae12b941c3facf8616ce62dd45bc6868076c2aeb82106f98123ea6c5bb64182"),
    (2652728736, "3e8ac315c2bcca3e83c5e6e25c5758a75c03732063a6ff0c1b0e24b9ff49f6e6"),
    (2573998176, "9c73b27c4343236790008979d5e0b565f687aa64440031b9e096fed1ab8039c5"),
    (2706402896, "1dccc19a3741972c661de734572dc7d4b899e6e99bfbdf3ae4e057c707e5b339"),
    (101535150936, "76a05106ece259be83d1bfea0dcb54f23377864ee60fd321690f35e6698a2af5"),
    (101537926640, "9dc51433e195bc78b5ad846b68643cc0351d3b174e5e4309222d3b7402c83afd"),
]
FIXTURE = pathlib.Path(__file__).resolve().parent.parent / "fixtures" / "deepseek" / "v41-manifest.coax"
# Dtype codes shared with psionic/lib/deepseek/flash-manifest.coax.
DTYPES = {"F32": (0, 4), "BF16": (1, 2), "F8_E4M3": (2, 1), "F8_E8M0": (3, 1), "I8": (4, 1)}


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
        length = struct.unpack("<Q", get(shard(i), 0, 7))[0]
        body = get(shard(i), 8, 8 + length - 1)
        if len(body) != length:
            raise SystemExit(f"{shard(i)}: short header read")
        (directory / f"header-{i + 1:05d}.json").write_bytes(body)
        print(f"{shard(i)}: header {length} bytes", flush=True)
    return 0


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load(directory: pathlib.Path):
    config = (directory / "config.json").read_bytes()
    index = (directory / "model.safetensors.index.json").read_bytes()
    headers = [(directory / f"header-{i + 1:05d}.json").read_bytes() for i in range(len(SHARDS))]
    problems = []
    if sha(config) != CONFIG_SHA256:
        problems.append(f"config.json sha256 {sha(config)}")
    if sha(index) != INDEX_SHA256:
        problems.append(f"index sha256 {sha(index)}")
    joined = sha(b"".join(headers))
    if joined != HEADERS_SHA256:
        problems.append(f"headers sha256 {joined}")
    tensors = {}
    for i, raw in enumerate(headers):
        header = json.loads(raw)
        header.pop("__metadata__", None)
        spans = []
        for name, t in header.items():
            if t["dtype"] not in DTYPES:
                problems.append(f"{name}: dtype {t['dtype']}")
                continue
            count = 1
            for d in t["shape"]:
                count *= d
            begin, end = t["data_offsets"]
            if end - begin != count * DTYPES[t["dtype"]][1]:
                problems.append(f"{name}: {end - begin} bytes for {t['shape']} {t['dtype']}")
            spans.append((begin, end))
            if name in tensors:
                problems.append(f"{name}: in two shards")
            tensors[name] = (i, t["dtype"], tuple(t["shape"]), end - begin)
        spans.sort()
        at = 0
        for begin, end in spans:
            if begin != at:
                problems.append(f"{shard(i)}: gap or overlap at {at}")
            at = end
        if 8 + len(raw) + at != SHARDS[i][0]:
            problems.append(f"{shard(i)}: tensors end at {8 + len(raw) + at}, shard is {SHARDS[i][0]}")
    mapping = json.loads(index)
    weight_map = mapping["weight_map"]
    if set(weight_map) != set(tensors):
        problems.append(f"index names {len(weight_map)} tensors, headers {len(tensors)}")
    for name, file in weight_map.items():
        if name in tensors and file != shard(tensors[name][0]):
            problems.append(f"{name}: index says {file}")
    total = sum(t[3] for t in tensors.values())
    if mapping["metadata"]["total_size"] != total:
        problems.append(f"index total {mapping['metadata']['total_size']}, headers {total}")
    return json.loads(config), tensors, problems


def rows(tensors):
    """Collapse experts per layer and kind, then identical rows across layers."""
    expert = re.compile(r"^(.*\.experts\.)(\d+)(\..*)$")
    grouped = {}
    for name, (_, dtype, shape, size) in tensors.items():
        m = expert.match(name)
        key = (m.group(1) + "*" + m.group(3)) if m else name
        g = grouped.setdefault(key, [dtype, shape, 0, 0, set(), True])
        if g[0] != dtype or g[1] != shape:
            g[5] = False
        g[2] += 1
        g[3] += size
        if m:
            g[4].add(int(m.group(2)))
    out = {}
    for key, (dtype, shape, count, size, ids, same) in grouped.items():
        if not same or (ids and ids != set(range(len(ids)))):
            raise SystemExit(f"{key}: experts differ in dtype or shape, or are not 0..E-1")
        m = re.match(r"^(layers|mtp|vision\.blocks)\.(\d+)\.(.*)$", key)
        if m:
            pattern = m.group(1) + ".#." + m.group(3)
            layer = int(m.group(2))
        else:
            pattern, layer = key, -1
        k = (pattern, dtype, shape, count)
        e = out.setdefault(k, [0, 0])
        if layer >= 0:
            e[0] |= 1 << layer
        e[1] += size
    return sorted(out.items(), key=lambda item: (item[0][0], -item[1][0]))


# DvConfig field, config.json section and key. Lists keep their length.
CONFIG_FIELDS = [
    ("layers", "text_config", "num_hidden_layers"), ("drafts", "text_config", "num_nextn_predict_layers"),
    ("width", "text_config", "hidden_size"), ("vocab", "text_config", "vocab_size"),
    ("heads", "text_config", "num_attention_heads"), ("kv_heads", "text_config", "num_key_value_heads"),
    ("head", "text_config", "head_dim"), ("rope", "text_config", "qk_rope_head_dim"),
    ("q_lora", "text_config", "q_lora_rank"), ("o_lora", "text_config", "o_lora_rank"), ("o_groups", "text_config", "o_groups"),
    ("experts", "text_config", "n_routed_experts"), ("shared", "text_config", "n_shared_experts"),
    ("k", "text_config", "num_experts_per_tok"), ("moe", "text_config", "moe_intermediate_size"),
    ("window", "text_config", "sliding_window"), ("ratios", "text_config", "compress_ratios"),
    ("kv_sources", "text_config", "kv_source_layer_ids"), ("index_sources", "text_config", "index_source_layer_ids"),
    ("candidate_source", "text_config", "candidate_source_layer_id"), ("candidate_blocks", "text_config", "candidate_topk_blocks"),
    ("candidate_block", "text_config", "candidate_block_size"), ("index_heads", "text_config", "index_n_heads"),
    ("index_dim", "text_config", "index_head_dim"), ("index_topk", "text_config", "index_topk"),
    ("hc", "text_config", "hc_mult"), ("sinkhorn", "text_config", "hc_sinkhorn_iters"),
    ("engram_layers", "text_config", "engram_layer_ids"), ("engram_rows", "text_config", "engram_num_embeddings"),
    ("engram_ngram", "text_config", "engram_max_ngram_size"), ("engram_heads", "text_config", "engram_n_heads"),
    ("engram_dim", "text_config", "engram_head_dim"), ("engram_vocab", "text_config", "engram_compressed_vocab_size"),
    ("draft_experts", "text_config", "dspark_n_routed_experts"), ("draft_k", "text_config", "dspark_num_experts_per_tok"),
    ("draft_targets", "text_config", "dspark_target_layer_ids"), ("markov", "text_config", "dspark_markov_rank"),
    ("block", "quantization_config", "weight_block_size"),
    ("vision_layers", "vision_config", "num_hidden_layers"), ("vision_width", "vision_config", "hidden_size"),
    ("vision_mlp", "vision_config", "intermediate_size"), ("patch", "vision_config", "patch_size"),
    ("downsample", "vision_config", "downsample_ratio"),
]


def config_lines(config) -> list:
    out = ["// The pinned config.json values each DvConfig field is copied from."]
    for field, section, key in CONFIG_FIELDS:
        value = config[section][key]
        name = "DV_PINNED_" + field.upper()
        if field == "draft_targets":
            value = len(value)
        elif field == "block":
            if value[0] != value[1]:
                raise SystemExit("weight blocks are not square")
            value = value[0]
        if isinstance(value, list):
            out.append(f"{name} :: [{len(value)}]int.[{','.join(map(str, value))}];")
        else:
            out.append(f"{name} :: {int(value)};")
    return out


def fixture(tensors, config) -> str:
    lines = [
        "// Generated by psionic/tests/deepseek_v41_manifest.py from the safetensors headers of",
        f"// {REPOSITORY} at {REVISION} (48 shards, read by range). Do not edit.",
        f"// config.json sha256 {CONFIG_SHA256}, model.safetensors.index.json sha256",
        f"// {INDEX_SHA256}, the 48 header JSON bytes in shard order sha256",
        f"// {HEADERS_SHA256}. No weights were downloaded.",
        "// One row per tensor kind: name with # for the layer and * for the expert, dtype code",
        "// (0 F32, 1 BF16, 2 F8_E4M3, 3 F8_E8M0, 4 I8), rank, shape, layer bit set (0 when",
        "// the name has no layer), tensors per layer, and total bytes of the row.",
        "DvRow :: struct {name:string;dtype:int;rank:int;shape:[4]int;layers:u64;count:int;bytes:int;}",
    ]
    collapsed = rows(tensors)
    lines.append(f"DV_MANIFEST_ROWS :: {len(collapsed)};")
    lines.append(f"DV_MANIFEST :: [{len(collapsed)}]DvRow.[")
    for (pattern, dtype, shape, count), (layers, size) in collapsed:
        dims = list(shape) + [0] * (4 - len(shape))
        lines.append(f'    DvRow.{{name="{pattern}",dtype={DTYPES[dtype][0]},rank={len(shape)},'
                     f'shape=[4]int.[{",".join(map(str, dims))}],layers=0x{layers:x},count={count},bytes={size}}},')
    lines.append("];")
    lines.append(f"DV_MANIFEST_TENSORS :: {len(tensors)};")
    total = sum(t[3] for t in tensors.values())
    lines.append(f"DV_MANIFEST_BYTES :: {total};")
    by = [0] * len(DTYPES)
    for _, dtype, _, size in tensors.values():
        by[DTYPES[dtype][0]] += size
    lines.append(f"DV_MANIFEST_DTYPE_BYTES :: [5]int.[{','.join(map(str, by))}];")
    lines.append(f"DV_MANIFEST_SHARD_BYTES :: {sum(s for s, _ in SHARDS)};")
    lines += config_lines(config)
    return "\n".join(lines) + "\n"


def main(argv) -> int:
    if len(argv) >= 2 and argv[0] == "--fetch":
        return fetch(pathlib.Path(argv[1]))
    if not argv:
        print(__doc__)
        return 2
    config, tensors, problems = load(pathlib.Path(argv[0]))
    for p in problems[:20]:
        print(p)
    if problems:
        return 1
    text = fixture(tensors, config)
    if "--write" in argv:
        FIXTURE.write_text(text)
        print(f"wrote {FIXTURE} ({len(text)} bytes, {len(tensors)} tensors)")
        return 0
    if not FIXTURE.exists() or FIXTURE.read_text() != text:
        print(f"{FIXTURE} differs from the headers; rerun with --write")
        return 1
    print(f"manifest ok: {len(tensors)} tensors, {sum(t[3] for t in tensors.values())} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
