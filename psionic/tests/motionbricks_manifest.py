"""Chapter 149's checkpoint inventory: the tensor names, dtypes and shapes of
the three pinned MotionBricks SOMA checkpoints, and the pose tokenizer's
codebook, written as Coax fixtures.

    python3 -I psionic/tests/motionbricks_manifest.py SOMA_DIRECTORY [--write]

SOMA_DIRECTORY is the soma/soma-v6-nova-4n8g-ea43dd8-fp32-v1 folder of
https://huggingface.co/nvidia/MotionBricks/tree/3cb362d9b95aa40462abcb4d8cfae9ffb6d97410
The script hashes each whole checkpoint against the pin, reads only the
safetensors header (JSON, at most 1 MiB) and the 5 x 16 x 32 F32 codebook
(pose_net.quantizer.vq._codebook.embed), and compares the fixtures it would
write with the committed ones; --write replaces them. It never imports or
runs anything from the download. Tooling only.
"""
import hashlib
import json
import pathlib
import struct
import sys

REVISION = "3cb362d9b95aa40462abcb4d8cfae9ffb6d97410"
STEP = "checkpoints/model-step=1500000.safetensors"
# (prefix, folder, bytes, sha256)
PINS = [
    ("VQVAE", "vqvae-v6-soma-nova-4n8g-ea43dd8-fp32-v1", 93442464,
     "ba0d1db48ba7182ef5d1a72268bfaa1a196f1f1fbf00144c046d947b90684a1a"),
    ("POSE", "mmm-v6-soma-pose-nova-4n8g-ea43dd8-fp32-v1", 546109841,
     "8537d9d33a880ff1df6c16c28ed061d60c461a5918d70b1cf99251ff91a10a10"),
    ("ROOT", "mmm-v6-soma-root-nova-4n8g-ea43dd8-fp32-v1", 161695084,
     "a48d48c3d8ff98c825ca315797644789ae37f2db9069779a3118f42e84f4f29e"),
]
FIXTURES = pathlib.Path(__file__).resolve().parent.parent / "fixtures"
MANIFEST = FIXTURES / "motionbricks-manifest.coax"
CODEBOOK = FIXTURES / "motionbricks-codebook.coax"
CODEBOOK_NAME = "pose_net.quantizer.vq._codebook.embed"


def read(path: pathlib.Path, size: int, sha: str):
    if path.stat().st_size != size:
        raise SystemExit(f"refused: {path} is {path.stat().st_size} bytes, the pin says {size}")
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 24), b""):
            digest.update(chunk)
    if digest.hexdigest() != sha:
        raise SystemExit(f"refused: sha256 {digest.hexdigest()} is not the pinned {sha}")
    with path.open("rb") as f:
        n = struct.unpack("<Q", f.read(8))[0]
        if not 2 <= n <= 1 << 20:
            raise SystemExit(f"refused: header of {n} bytes")
        header = json.loads(f.read(n))
    return n, header


def main() -> int:
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != "--write"):
        print(__doc__.strip().splitlines()[4].strip(), file=sys.stderr)
        return 2
    root = pathlib.Path(sys.argv[1])
    manifest = [
        "// Chapter 149's checkpoint inventory, written by psionic/tests/motionbricks_manifest.py\n",
        f"// from nvidia/MotionBricks at {REVISION}, soma/soma-v6-nova-4n8g-ea43dd8-fp32-v1.\n",
        "// One line a tensor, sorted by name: name, dtype, dimensions. No weights.\n",
        f'MB_PINNED_REVISION :: "{REVISION}";\n',
    ]
    codebook = None
    for prefix, folder, size, sha in PINS:
        path = root / folder / "version_0" / STEP
        n, header = read(path, size, sha)
        lines = []
        for name in sorted(header):
            if name == "__metadata__":
                continue
            t = header[name]
            lines.append(f"{name} {t['dtype']} {','.join(str(d) for d in t['shape'])}")
        text = "\\n".join(lines) + "\\n"
        manifest.append(f"// {folder}/version_0/{STEP}, {size} bytes.\n")
        manifest.append(f'MB_{prefix}_SHA256 :: "{sha}";\n')
        manifest.append(f"MB_{prefix}_BYTES :: {size};\n")
        manifest.append(f"MB_{prefix}_TENSORS :: {len(lines)};\n")
        manifest.append(f'MB_{prefix}_MANIFEST :: "{text}";\n')
        if prefix == "VQVAE":
            t = header[CODEBOOK_NAME]
            if t["dtype"] != "F32" or t["shape"] != [5, 16, 32]:
                raise SystemExit(f"refused: {CODEBOOK_NAME} is not F32 5 x 16 x 32")
            begin, end = t["data_offsets"]
            with path.open("rb") as f:
                f.seek(8 + n + begin)
                raw = f.read(end - begin)
            words = struct.unpack("<2560I", raw)
            rows = []
            for r in range(0, 2560, 8):
                rows.append("    " + ",".join(f"0x{w:08x}" for w in words[r:r + 8]) + ",\n")
            codebook = (
                "// Chapter 149's real-weight slice, written by psionic/tests/motionbricks_manifest.py:\n"
                f"// {CODEBOOK_NAME} of the pinned SOMA pose VQ-VAE\n"
                f"// (nvidia/MotionBricks at {REVISION}, sha256 {sha}),\n"
                "// 5 heads x 16 codes x 32 values, F32 bit patterns, head-major.\n"
                "MB_SOMA_CODEBOOK_BITS :: [2560]u32.[\n" + "".join(rows) + "];\n"
            )
    out = "".join(manifest)
    if len(sys.argv) == 3:
        MANIFEST.write_text(out)
        CODEBOOK.write_text(codebook)
        print(f"wrote {MANIFEST} and {CODEBOOK}")
        return 0
    if MANIFEST.read_text() != out or CODEBOOK.read_text() != codebook:
        print("the fixtures differ from the pinned checkpoints", file=sys.stderr)
        return 1
    print("3 checkpoints hashed; manifests and codebook match the pinned SOMA package")
    return 0


if __name__ == "__main__":
    sys.exit(main())
