"""Chapter 153's checkpoint inventory: the tensor names, dtypes and shapes of
the pinned Parakeet TDT checkpoint, written as a Coax fixture.

    python3 -I psionic/tests/parakeet_manifest.py MODEL_DIRECTORY [--write]

MODEL_DIRECTORY holds model.safetensors downloaded from
https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3/resolve/541d1f99c6b0c3cd0b11a95167540bb8edefd82b/model.safetensors
The script hashes the whole file against the pin, reads only the
safetensors header (JSON, at most 1 MiB), and compares the fixture it would
write with the committed one; --write replaces it. It never imports or runs
anything from the download. Tooling only.
"""
import hashlib
import json
import pathlib
import struct
import sys

REVISION = "541d1f99c6b0c3cd0b11a95167540bb8edefd82b"
SHA256 = "3a2026366188c8c68598edbbff92f8d11590a08e0ae2e6775544e7b07d6a5e11"
SIZE = 2508311120
FIXTURE = pathlib.Path(__file__).resolve().parent.parent / "fixtures" / "parakeet-tdt-manifest.coax"


def main() -> int:
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != "--write"):
        print(__doc__.strip().splitlines()[3].strip(), file=sys.stderr)
        return 2
    path = pathlib.Path(sys.argv[1]) / "model.safetensors"
    if path.stat().st_size != SIZE:
        print(f"refused: {path} is {path.stat().st_size} bytes, the pin says {SIZE}", file=sys.stderr)
        return 1
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 24), b""):
            digest.update(chunk)
    if digest.hexdigest() != SHA256:
        print(f"refused: sha256 {digest.hexdigest()} is not the pinned {SHA256}", file=sys.stderr)
        return 1
    with path.open("rb") as f:
        n = struct.unpack("<Q", f.read(8))[0]
        if not 2 <= n <= 1 << 20:
            print(f"refused: header of {n} bytes", file=sys.stderr)
            return 1
        header = json.loads(f.read(n))
        # The decoder starts from the blank's embedding row and assumes it
        # is zero (NeMo's blank-as-padding): row 8192 of 8193 x 640 F32.
        embedding = header["decoder.embedding.weight"]
        if embedding["dtype"] != "F32" or embedding["shape"] != [8193, 640]:
            print("refused: decoder.embedding.weight is not F32 8193 x 640", file=sys.stderr)
            return 1
        f.seek(8 + n + embedding["data_offsets"][0] + 8192 * 640 * 4)
        if any(f.read(640 * 4)):
            print("refused: the blank's embedding row is not zero", file=sys.stderr)
            return 1
    lines = []
    for name in sorted(header):
        if name == "__metadata__":
            continue
        t = header[name]
        lines.append(f"{name} {t['dtype']} {','.join(str(d) for d in t['shape'])}")
    text = "\\n".join(lines) + "\\n"
    out = (
        "// Chapter 153's checkpoint inventory, written by psionic/tests/parakeet_manifest.py\n"
        f"// from nvidia/parakeet-tdt-0.6b-v3 at {REVISION},\n"
        f"// model.safetensors, {SIZE} bytes, sha256 {SHA256}.\n"
        "// One line a tensor, sorted by name: name, dtype, dimensions. No weights.\n"
        f'TDT_PINNED_REVISION :: "{REVISION}";\n'
        f'TDT_PINNED_SHA256 :: "{SHA256}";\n'
        f"TDT_PINNED_TENSORS :: {len(lines)};\n"
        f'TDT_PINNED_MANIFEST :: "{text}";\n'
    )
    if len(sys.argv) == 3:
        FIXTURE.write_text(out)
        print(f"wrote {FIXTURE} ({len(lines)} tensors)")
        return 0
    if FIXTURE.read_text() != out:
        print(f"{FIXTURE} differs from the pinned checkpoint's header", file=sys.stderr)
        return 1
    print(f"{len(lines)} tensors, the blank embedding row zero: the fixture matches the pinned checkpoint")
    return 0


if __name__ == "__main__":
    sys.exit(main())
