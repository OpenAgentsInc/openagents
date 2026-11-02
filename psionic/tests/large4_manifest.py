# Firm reason: 2 (independent oracle: reads the shard headers outside Coax, which derives the same lines from params.json and compares)
"""Chapter 198's checkpoint inventory: every tensor of the pinned Mistral
Large 4 checkpoint (54 shards), grouped by kind, written as a Coax fixture
from the safetensors headers alone.

    python3 -I psionic/tests/large4_manifest.py --fetch DIRECTORY
    python3 -I psionic/tests/large4_manifest.py DIRECTORY [--write]

The checkpoint is 54 shards and 1,060.6 GB, far beyond this host's disk, so
--fetch downloads only params.json, consolidated.safetensors.index.json and,
by HTTP range requests, the first 8 bytes and the JSON header of each shard
(about 6 MB in all) from
https://huggingface.co/mistralai/Mistral-Large-4-1T-A52B/resolve/395730aefd2f928daaf7b69fbd9fefab0ee69b64/
It reads each shard's size from the Content-Range of the first request.

The second form hashes params.json, the index and the 54 headers against the
pins, checks that each header's tensors tile its shard exactly up to the
pinned shard size, that the index places every tensor in the shard whose
header holds it and nothing else, that the tensors add up to the index's
total_size, and compares the fixture it would write with the committed one;
--write replaces it.

A tensor kind is one name with its layer and expert numbers taken out, one
dtype and one shape. Its line names the layers (and experts) that hold it as
ranges: "layers.0-16,19-20.attention.indexer.wk.weight BF16 128,6144". The
Coax inventory (psionic/lib/large4-mesh.coax) derives the same lines from
params.json's numbers and compares them. A header read by range cannot be
checked against the shard's sha256 (that needs the whole shard); its pin is
the hash observed on first fetch. It never imports or runs anything from the
download. Tooling only.
"""
import hashlib
import json
import pathlib
import re
import struct
import sys
import urllib.request

REPOSITORY = "mistralai/Mistral-Large-4-1T-A52B"
REVISION = "395730aefd2f928daaf7b69fbd9fefab0ee69b64"
BASE = f"https://huggingface.co/{REPOSITORY}/resolve/{REVISION}/"
PARAMS_SHA256 = "dc6617d800bacc9d3caa41570f885467f733a7cee25f8eef3a60e01c1ff98343"
INDEX_SHA256 = "dda188a5487ea7eaf23d71c8eee21d83cf1c88b3558a8198e00a5a1507d62b70"
SHARDS = 54
# sha256 of the 54 headers' JSON bytes, concatenated in shard order, and of
# the shard sizes written one a line; both observed on first fetch.
HEADERS_SHA256 = "8c08aa0f3d1ff0b4274a90e446f0510fac52d7feac68f1d9cb6e943094751169"
SIZES_SHA256 = "4ae0f68eef7c6f38b1b07040948e41dab78200ea3a580259c1c32ccbcc212e29"
FIXTURE = pathlib.Path(__file__).resolve().parent.parent / "fixtures" / "mistral-large4-manifest.coax"
DTYPE_BYTES = {"BF16": 2, "F32": 4, "F8_E4M3": 1, "U8": 1, "F8_E8M0": 1}


def shard(i: int) -> str:
    return f"consolidated-{i + 1:05d}-of-{SHARDS:05d}.safetensors"


def get(name: str, first: int | None = None, last: int | None = None) -> tuple[bytes, str]:
    request = urllib.request.Request(BASE + name)
    if first is not None:
        request.add_header("Range", f"bytes={first}-{last}")
    with urllib.request.urlopen(request) as response:
        return response.read(), response.headers.get("Content-Range", "")


def fetch(directory: pathlib.Path) -> int:
    directory.mkdir(parents=True, exist_ok=True)
    for name in ("params.json", "consolidated.safetensors.index.json"):
        (directory / name).write_bytes(get(name)[0])
    sizes = []
    for i in range(SHARDS):
        head, span = get(shard(i), 0, 7)
        n = struct.unpack("<Q", head)[0]
        if not 2 <= n <= 1 << 24 or not re.fullmatch(r"bytes 0-7/\d+", span):
            print(f"refused: {shard(i)} header of {n} bytes, range {span!r}", file=sys.stderr)
            return 1
        sizes.append(int(span.split("/")[1]))
        body = get(shard(i), 8, 7 + n)[0]
        if len(body) != n:
            print(f"refused: {shard(i)} header came back {len(body)} bytes, not {n}", file=sys.stderr)
            return 1
        (directory / f"header-{i + 1:02d}.json").write_bytes(body)
    (directory / "sizes.txt").write_text("".join(f"{s}\n" for s in sizes))
    print(f"fetched params, index and {SHARDS} headers into {directory}")
    return 0


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def ranges(numbers: list[int]) -> str:
    out = []
    start = prev = numbers[0]
    for n in numbers[1:] + [None]:
        if n is not None and n == prev + 1:
            prev = n
            continue
        out.append(f"{start}" if start == prev else f"{start}-{prev}")
        if n is not None:
            start = prev = n
    return ",".join(out)


KIND = re.compile(r"^(layers|vision_encoder\.transformer\.layers|mtp_dspark\.backbone\.layers)\.(\d+)\.(experts\.(\d+)\.)?(.*)$")


def build(directory: pathlib.Path) -> str:
    params = (directory / "params.json").read_bytes()
    index = (directory / "consolidated.safetensors.index.json").read_bytes()
    headers = [(directory / f"header-{i + 1:02d}.json").read_bytes() for i in range(SHARDS)]
    sizes_text = (directory / "sizes.txt").read_bytes()
    for name, data, pin in (("params.json", params, PARAMS_SHA256), ("index", index, INDEX_SHA256),
                            ("headers", b"".join(headers), HEADERS_SHA256), ("sizes", sizes_text, SIZES_SHA256)):
        if sha(data) != pin:
            raise SystemExit(f"{name}: sha256 {sha(data)} is not the pin {pin}")
    sizes = [int(s) for s in sizes_text.split()]
    index_json = json.loads(index)
    weights = index_json["weight_map"]
    seen = {}
    total = 0
    for i, body in enumerate(headers):
        table = json.loads(body)
        table.pop("__metadata__", None)
        end = 0
        for name in sorted(table, key=lambda k: table[k]["data_offsets"][0]):
            t = table[name]
            first, last = t["data_offsets"]
            size = DTYPE_BYTES[t["dtype"]]
            for d in t["shape"]:
                size *= d
            if first != end or last - first != size:
                raise SystemExit(f"{shard(i)} {name}: offsets {first}..{last} do not tile the shard")
            end = last
            if weights.get(name) != shard(i) or name in seen:
                raise SystemExit(f"{name}: the index does not place it in {shard(i)} alone")
            seen[name] = (t["dtype"], tuple(t["shape"]))
        if 8 + len(body) + end != sizes[i]:
            raise SystemExit(f"{shard(i)}: the tensors do not fill the shard")
        total += end
    if sorted(seen) != sorted(weights):
        raise SystemExit("the headers and the index name different tensors")
    if total != index_json["metadata"]["total_size"]:
        raise SystemExit(f"the tensors hold {total} bytes, not the index's total_size")
    kinds: dict[tuple, tuple[set, set]] = {}
    for name, (dtype, shape) in seen.items():
        m = KIND.match(name)
        if m:
            key = (m.group(1), m.group(3) is not None, m.group(5), dtype, shape)
            layer_set, expert_set = kinds.setdefault(key, (set(), set()))
            layer_set.add(int(m.group(2)))
            if m.group(4) is not None:
                expert_set.add(int(m.group(4)))
        else:
            kinds.setdefault((name, False, "", dtype, shape), (set(), set()))
    lines = []
    for (prefix, expert, rest, dtype, shape), (layer_set, expert_set) in kinds.items():
        if layer_set:
            name = f"{prefix}.{ranges(sorted(layer_set))}." + (f"experts.{ranges(sorted(expert_set))}." if expert else "") + rest
            count = sum(1 for k, v in seen.items() if (m := KIND.match(k)) and m.group(1) == prefix
                        and (m.group(3) is not None) == expert and m.group(5) == rest and v == (dtype, shape))
            if count != len(layer_set) * (len(expert_set) if expert else 1):
                raise SystemExit(f"{name}: the experts differ between layers")
        else:
            name = prefix
        dims = ",".join(str(d) for d in shape) or "-"
        lines.append(f"{name} {dtype} {dims}")
    lines.sort()
    p = json.loads(params)
    manifest = "\\n".join(lines) + "\\n"
    return (
        "// Chapter 198's checkpoint inventory, written by psionic/tests/large4_manifest.py\n"
        f"// from {REPOSITORY} at {REVISION}: the headers of its {SHARDS} shards\n"
        f"// (headers sha256 {HEADERS_SHA256}),\n"
        f"// params.json sha256 {PARAMS_SHA256},\n"
        f"// consolidated.safetensors.index.json sha256 {INDEX_SHA256}. No weights were downloaded.\n"
        "// One line a tensor kind, sorted: the name with its layers (and experts) as ranges, dtype, dimensions.\n"
        f"// params.json: dim {p['dim']}, n_layers {p['n_layers']}, n_heads {p['n_heads']}, vocab_size {p['vocab_size']},\n"
        f"// moe {json.dumps(p['moe'], sort_keys=True)}\n"
        f"L4_PINNED_REVISION :: \"{REVISION}\";\n"
        f"L4_PINNED_SHARDS :: {SHARDS};\n"
        f"L4_PINNED_FILE_BYTES :: {sum(sizes)};\n"
        f"L4_PINNED_TENSOR_BYTES :: {total};\n"
        f"L4_PINNED_TENSORS :: {len(seen)};\n"
        f"L4_PINNED_KINDS :: {len(lines)};\n"
        f"L4_PINNED_MANIFEST :: \"{manifest}\";\n"
    )


def main() -> int:
    if len(sys.argv) >= 3 and sys.argv[1] == "--fetch":
        return fetch(pathlib.Path(sys.argv[2]))
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    text = build(pathlib.Path(sys.argv[1]))
    if "--write" in sys.argv[2:]:
        FIXTURE.write_text(text)
        return 0
    if FIXTURE.read_text() != text:
        print("the committed fixture differs from the headers", file=sys.stderr)
        return 1
    print(f"{FIXTURE.name}: matches the pinned headers")
    return 0


if __name__ == "__main__":
    sys.exit(main())
