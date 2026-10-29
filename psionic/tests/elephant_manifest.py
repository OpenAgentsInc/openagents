#!/usr/bin/env python3
"""Build/check a reviewable pinned inventory from the unmodified Elephant file."""
import hashlib
import json
import struct
import sys
from pathlib import Path

REVISION = "b3f8a93462fb1051a9de031e94a7ff380e254072"
SHA = "682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba"
SIZE = 18819722392
MANIFEST_SHA = "4744758962cc73b7e6954c4385cda209ea29c3d9247f524fa14855ca245eedc3"
ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / "psionic/fixtures/models/elephant/manifest.json"

def make(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        n = struct.unpack("<Q", f.read(8))[0]
        if not 2 <= n <= 1048576:
            raise ValueError("header size")
        pairs = json.loads(f.read(n), object_pairs_hook=lambda v: v)
        f.seek(0)
        for chunk in iter(lambda: f.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    if path.stat().st_size != SIZE or digest.hexdigest() != SHA:
        raise ValueError("checkpoint is not the pinned original")
    tensors = []
    for name, properties in pairs:
        if name == "__metadata__":
            continue
        p = dict(properties)
        if len(p) != len(properties) or p.keys() != {"dtype", "shape", "data_offsets"}:
            raise ValueError("tensor fields")
        if p["dtype"] != "BF16" or not 0 <= len(p["shape"]) <= 5 or any(not isinstance(v, int) or v <= 0 for v in p["shape"]):
            raise ValueError("tensor profile")
        tensors.append({"name": name, "dtype": p["dtype"], "shape": p["shape"], "shard": 0})
    if len(tensors) != 760 or len({v["name"] for v in tensors}) != 760:
        raise ValueError("inventory")
    tensors.sort(key=lambda v: v["name"])
    return {"model":"baselabs/elephant-1", "revision":REVISION, "file":"model.safetensors", "bytes":SIZE, "sha256":SHA, "tensors":tensors}

def header():
    raw = TARGET.read_bytes()
    if hashlib.sha256(raw).hexdigest() != MANIFEST_SHA:
        raise ValueError("pinned inventory fixture changed")
    manifest = json.loads(raw)
    print("/* Generated from the pinned Elephant inventory. */")
    print("static const struct {const char *name; int64_t rank; int64_t dims[5];} elephant_specs[] = {")
    for tensor in manifest["tensors"]:
        shape = tensor["shape"]
        dims = shape + [0] * (5 - len(shape))
        print("  {" + json.dumps(tensor["name"]) + "," + str(len(shape)) + ",{ " + ",".join(map(str,dims)) + " }},")
    print("};")

def main():
    if sys.argv[1:] == ["--header"]:
        header()
        return
    if len(sys.argv) != 3 or sys.argv[1] not in ("--write", "--check"):
        raise SystemExit("usage: elephant_manifest.py --header OR --write|--check PATH_TO_ORIGINAL")
    data = json.dumps(make(Path(sys.argv[2])), indent=2) + "\n"
    if sys.argv[1] == "--write":
        TARGET.write_text(data)
    elif TARGET.read_text() != data:
        raise SystemExit("manifest differs from pinned original")
    print("pinned Elephant digest and 760-tensor inventory match")

if __name__ == "__main__":
    main()
