"""Chapter 163's checkpoint inventory: the tensor names, dtypes and shapes of
the pinned Nemotron 3 Nano 4B checkpoint, written as a Coax fixture.

    python3 -I psionic/tests/nemotron_manifest.py MODEL_DIRECTORY [--write]

MODEL_DIRECTORY holds config.json and model.safetensors downloaded from
https://huggingface.co/nvidia/NVIDIA-Nemotron-3-Nano-4B-BF16/resolve/dfaf35de3e30f1867dd8dbc38a7fc9fb52d3914f/
The script hashes both files against the pins, reads only the safetensors
header (JSON, at most 1 MiB), and compares the fixture it would write with
the committed one; --write replaces it. It never imports or runs anything
from the download. Tooling only.
"""
import hashlib
import json
import pathlib
import struct
import sys

REVISION = "dfaf35de3e30f1867dd8dbc38a7fc9fb52d3914f"
SHA256 = "55d4e2519456c4a9bddf596b0748d630e3b2ce6ff6f4c2b7ed3e07e2b00dad42"
SIZE = 7947142640
CONFIG_SHA256 = "fde9241f66cd414458df444a80eb535f53aef2b4b240a91f092e651fa6f27219"
FIXTURE = pathlib.Path(__file__).resolve().parent.parent / "fixtures" / "nemotron-nano-manifest.coax"


def digest(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 24), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != "--write"):
        print(__doc__.strip().splitlines()[3].strip(), file=sys.stderr)
        return 2
    directory = pathlib.Path(sys.argv[1])
    config = directory / "config.json"
    if digest(config) != CONFIG_SHA256:
        print(f"refused: {config} does not hash to the pin", file=sys.stderr)
        return 1
    path = directory / "model.safetensors"
    if path.stat().st_size != SIZE:
        print(f"refused: {path} is {path.stat().st_size} bytes, the pin says {SIZE}", file=sys.stderr)
        return 1
    if digest(path) != SHA256:
        print(f"refused: {path} does not hash to the pin", file=sys.stderr)
        return 1
    with path.open("rb") as f:
        n = struct.unpack("<Q", f.read(8))[0]
        if not 2 <= n <= 1 << 20:
            print(f"refused: header of {n} bytes", file=sys.stderr)
            return 1
        header = json.loads(f.read(n))
    # The config fields the chapter's shape is built from, copied verbatim.
    fields = json.loads(config.read_text())
    keys = ["hidden_size", "vocab_size", "num_hidden_layers", "mamba_num_heads", "mamba_head_dim",
            "ssm_state_size", "n_groups", "conv_kernel", "num_attention_heads", "num_key_value_heads",
            "head_dim", "intermediate_size", "layer_norm_epsilon", "rms_norm_eps", "hybrid_override_pattern",
            "mamba_hidden_act", "mlp_hidden_act", "use_conv_bias", "use_bias", "mamba_proj_bias",
            "attention_bias", "mlp_bias", "tie_word_embeddings"]
    settings = "\\n".join(f"{k} {json.dumps(fields[k])}".replace('"', "'") for k in keys) + "\\n"
    lines = []
    for name in sorted(header):
        if name == "__metadata__":
            continue
        t = header[name]
        lines.append(f"{name} {t['dtype']} {','.join(str(d) for d in t['shape'])}")
    text = "\\n".join(lines) + "\\n"
    out = (
        "// Chapter 163's checkpoint inventory, written by psionic/tests/nemotron_manifest.py\n"
        f"// from nvidia/NVIDIA-Nemotron-3-Nano-4B-BF16 at {REVISION},\n"
        f"// model.safetensors, {SIZE} bytes, sha256 {SHA256};\n"
        f"// config.json sha256 {CONFIG_SHA256}.\n"
        "// One line a tensor, sorted by name: name, dtype, dimensions. No weights.\n"
        "// NH_PINNED_CONFIG copies the config fields the shape is built from\n"
        "// (JSON values, double quotes written as single quotes).\n"
        f'NH_PINNED_REVISION :: "{REVISION}";\n'
        f'NH_PINNED_SHA256 :: "{SHA256}";\n'
        f'NH_PINNED_CONFIG :: "{settings}";\n'
        f"NH_PINNED_TENSORS :: {len(lines)};\n"
        f'NH_PINNED_MANIFEST :: "{text}";\n'
    )
    if len(sys.argv) == 3:
        FIXTURE.write_text(out)
        print(f"wrote {FIXTURE} ({len(lines)} tensors)")
        return 0
    if FIXTURE.read_text() != out:
        print(f"{FIXTURE} differs from the pinned checkpoint's header", file=sys.stderr)
        return 1
    print(f"{len(lines)} tensors: the fixture matches the pinned checkpoint")
    return 0


if __name__ == "__main__":
    sys.exit(main())
