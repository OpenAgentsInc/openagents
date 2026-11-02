"""Chapter 87's draft-head inventory: the tensor names, dtypes and shapes of
Mistral Large 4's embedded draft head (`mtp_dspark.*`), written as a Coax
fixture, from one safetensors header alone.

    python3 -I psionic/tests/large4_draft_head_manifest.py --fetch DIRECTORY
    python3 -I psionic/tests/large4_draft_head_manifest.py DIRECTORY [--write]

The checkpoint is 54 shards and 1,060.6 GB, far beyond this host's disk, so
--fetch downloads only params.json, consolidated.safetensors.index.json and,
by HTTP range request, the first 8 bytes and the JSON header of the last
shard (about 4.8 MB in all) from
https://huggingface.co/mistralai/Mistral-Large-4-1T-A52B/resolve/395730aefd2f928daaf7b69fbd9fefab0ee69b64/

The second form hashes the three files against the pins, checks that the
index places every `mtp_dspark.` tensor, and nothing else, in the last
shard, that the header holds exactly those tensors, that they tile the shard
up to its pinned size, and compares the fixture it would write with the
committed one; --write replaces it.

A header read by range cannot be checked against the shard's sha256 (that
needs the whole shard); its pin is the hash observed on first fetch. It never
imports or runs anything from the download. Tooling only.
"""
import hashlib
import json
import pathlib
import struct
import sys
import urllib.request

REPOSITORY = "mistralai/Mistral-Large-4-1T-A52B"
REVISION = "395730aefd2f928daaf7b69fbd9fefab0ee69b64"
BASE = f"https://huggingface.co/{REPOSITORY}/resolve/{REVISION}/"
PARAMS_SHA256 = "dc6617d800bacc9d3caa41570f885467f733a7cee25f8eef3a60e01c1ff98343"
INDEX_SHA256 = "dda188a5487ea7eaf23d71c8eee21d83cf1c88b3558a8198e00a5a1507d62b70"
SHARD = "consolidated-00054-of-00054.safetensors"
SHARD_BYTES = 4199942840
# The Git LFS object id of the shard at the pinned revision, for a later full download.
SHARD_SHA256 = "4e369d0365bb15e42d5ccd4701e326f10514c3e1cd63812161f743a8911cde79"
HEADER_SHA256 = "e3bdee175f3cffef43e59213ccf1a28d11da06309d880cd6fa701b0967a43b63"
PREFIX = "mtp_dspark."
FIXTURE = pathlib.Path(__file__).resolve().parent.parent / "fixtures" / "mistral-large4-draft-head.coax"
DTYPE_BYTES = {"BF16": 2}


def get(name: str, first: int | None = None, last: int | None = None) -> bytes:
    request = urllib.request.Request(BASE + name)
    if first is not None:
        request.add_header("Range", f"bytes={first}-{last}")
    with urllib.request.urlopen(request) as response:
        return response.read()


def fetch(directory: pathlib.Path) -> int:
    directory.mkdir(parents=True, exist_ok=True)
    for name in ("params.json", "consolidated.safetensors.index.json"):
        (directory / name).write_bytes(get(name))
    (length,) = struct.unpack("<Q", get(SHARD, 0, 7))
    (directory / "header.bin").write_bytes(get(SHARD, 0, 7 + length))
    return 0


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build(directory: pathlib.Path) -> str:
    params = (directory / "params.json").read_bytes()
    index = (directory / "consolidated.safetensors.index.json").read_bytes()
    header = (directory / "header.bin").read_bytes()
    for name, data, pin in (("params.json", params, PARAMS_SHA256), ("index", index, INDEX_SHA256), ("header", header, HEADER_SHA256)):
        if sha(data) != pin:
            raise SystemExit(f"{name}: sha256 {sha(data)} is not the pin {pin}")
    (length,) = struct.unpack("<Q", header[:8])
    table = json.loads(header[8:8 + length])
    table.pop("__metadata__", None)
    weights = json.loads(index)["weight_map"]
    drafted = sorted(k for k in weights if k.startswith(PREFIX))
    if any(weights[k] != SHARD for k in drafted) or sorted(k for k, v in weights.items() if v == SHARD) != drafted:
        raise SystemExit("the index does not hold the draft head alone in the last shard")
    if sorted(table) != drafted:
        raise SystemExit("the header's tensors are not the index's draft-head tensors")
    end = 0
    lines = []
    for name in sorted(table, key=lambda k: table[k]["data_offsets"][0]):
        t = table[name]
        first, last = t["data_offsets"]
        size = DTYPE_BYTES[t["dtype"]]
        for d in t["shape"]:
            size *= d
        if first != end or last - first != size:
            raise SystemExit(f"{name}: offsets {first}..{last} do not tile the shard")
        end = last
    if 8 + length + end != SHARD_BYTES:
        raise SystemExit("the tensors do not fill the shard")
    for name in drafted:
        t = table[name]
        lines.append(f"{name[len(PREFIX):]} {t['dtype']} {','.join(str(d) for d in t['shape'])}")
    mtp = json.loads(params)["mtp"]
    manifest = "\\n".join(lines) + "\\n"
    return (
        "// Chapter 87's draft-head inventory, written by psionic/tests/large4_draft_head_manifest.py\n"
        f"// from {REPOSITORY} at {REVISION}: the header of {SHARD}\n"
        f"// ({SHARD_BYTES} bytes; header sha256 {HEADER_SHA256}),\n"
        f"// params.json sha256 {PARAMS_SHA256},\n"
        f"// consolidated.safetensors.index.json sha256 {INDEX_SHA256}. No weights were downloaded.\n"
        "// One line a tensor, sorted by name, without the \"mtp_dspark.\" prefix: name, dtype, dimensions.\n"
        f"// params.json's mtp section: {json.dumps(mtp, sort_keys=True)}\n"
        f"DH_PINNED_REVISION :: \"{REVISION}\";\n"
        f"DH_PINNED_SHARD_BYTES :: {SHARD_BYTES};\n"
        f"DH_PINNED_HEADER_BYTES :: {8 + length};\n"
        f"DH_PINNED_TENSORS :: {len(lines)};\n"
        f"DH_PINNED_MANIFEST :: \"{manifest}\";\n"
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
    print(f"{FIXTURE.name}: matches the pinned header")
    return 0


if __name__ == "__main__":
    sys.exit(main())
