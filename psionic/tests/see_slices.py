"""Independent check of chapter 143's original-weight vision slices.

    python3 -I psionic/tests/see_slices.py CONFIG.json SAFETENSORS OUT.f32

Reads the safetensors header itself (header only, no hashing), checks that the
model.visual tensors are exactly those the config implies, reads the slice's
tensors from the file, recomputes the slice with see_oracle.py's binary64
functions and compares Coax's f32 outputs. Tolerance, frozen before the first
comparison: |coax - oracle| <= 1e-4 * (1 + |oracle|) for every value.
"""
import array
import importlib.util
import json
import os
import struct
import sys

TOLERANCE = 1e-4
here = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("see_oracle", os.path.join(here, "see_oracle.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def expected(v):
    d, k = v["hidden_size"], v["spatial_merge_size"] ** 2 * v["hidden_size"]
    p, t, out, inner = v["patch_size"], v["temporal_patch_size"], v["out_hidden_size"], v["intermediate_size"]
    e = {"patch_embed.proj.weight": [d, v["in_channels"], t, p, p], "patch_embed.proj.bias": [d],
         "pos_embed.weight": [v["num_position_embeddings"], d], "merger.norm.weight": [d], "merger.norm.bias": [d],
         "merger.linear_fc1.weight": [k, k], "merger.linear_fc1.bias": [k], "merger.linear_fc2.weight": [out, k],
         "merger.linear_fc2.bias": [out]}
    for b in range(v["depth"]):
        for name, shape in (("norm1.weight", [d]), ("norm1.bias", [d]), ("attn.qkv.weight", [3 * d, d]), ("attn.qkv.bias", [3 * d]),
                            ("attn.proj.weight", [d, d]), ("attn.proj.bias", [d]), ("norm2.weight", [d]), ("norm2.bias", [d]),
                            ("mlp.linear_fc1.weight", [inner, d]), ("mlp.linear_fc1.bias", [inner]),
                            ("mlp.linear_fc2.weight", [d, inner]), ("mlp.linear_fc2.bias", [d])):
            e[f"blocks.{b}.{name}"] = shape
    return {"model.visual." + n: s for n, s in e.items()}


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    v = json.load(open(sys.argv[1]))["vision_config"]
    f = open(sys.argv[2], "rb")
    n = struct.unpack("<Q", f.read(8))[0]
    header = json.loads(f.read(n))
    visual = {k: t for k, t in header.items() if k.startswith("model.visual.")}
    want = expected(v)
    bad = [k for k in want if k not in visual or visual[k]["shape"] != want[k] or visual[k]["dtype"] != "BF16"]
    extra = [k for k in visual if k not in want]
    params = sum(eval_count(s) for s in want.values())
    print(f"oracle inventory: {len(visual)} model.visual tensors, {len(want)} expected, {len(bad)} wrong, {len(extra)} extra, {params} parameters")
    if bad or extra:
        sys.exit("inventory mismatch: " + ", ".join((bad + extra)[:5]))

    def read(name):
        t = header["model.visual." + name]
        a, b = t["data_offsets"]
        f.seek(8 + n + a)
        raw = f.read(b - a)
        wide = bytearray(2 * len(raw))
        wide[2::4], wide[3::4] = raw[0::2], raw[1::2]
        return array.array("f", bytes(wide)).tolist()

    d, heads, merge, patch, temporal = v["hidden_size"], v["num_heads"], v["spatial_merge_size"], v["patch_size"], v["temporal_patch_size"]
    side = int(round(v["num_position_embeddings"] ** 0.5))
    width, height = 64, 32
    pixels = [(i * 37 + 11) % 256 for i in range(width * height * 3)]
    rows, gh, gw = oracle.processor_patches(pixels, height, width, patch, temporal, merge)
    emb = [[a + b for a, b in zip(r, pos)] for r, pos in
           zip(oracle.linear(rows, read("patch_embed.proj.weight"), read("patch_embed.proj.bias"), d),
               oracle.pos_embed(read("pos_embed.weight"), side, d, gh, gw, merge))]
    names = ["norm1.weight", "norm1.bias", "attn.qkv.weight", "attn.qkv.bias", "attn.proj.weight", "attn.proj.bias",
             "norm2.weight", "norm2.bias", "mlp.linear_fc1.weight", "mlp.linear_fc1.bias", "mlp.linear_fc2.weight", "mlp.linear_fc2.bias"]
    keys = ["norm1", "norm1_bias", "qkv", "qkv_bias", "proj", "proj_bias", "norm2", "norm2_bias", "fc1", "fc1_bias", "fc2", "fc2_bias"]
    bp = {k: read("blocks.0." + nm) for k, nm in zip(keys, names)}
    blocked = oracle.block(emb, bp, gh, gw, merge, heads, v["rope_parameters"]["rope_theta"], 1e-6)
    mp = {"norm": read("merger.norm.weight"), "norm_bias": read("merger.norm.bias"), "fc1": read("merger.linear_fc1.weight"),
          "fc1_bias": read("merger.linear_fc1.bias"), "fc2": read("merger.linear_fc2.weight"), "fc2_bias": read("merger.linear_fc2.bias")}
    merged = oracle.merger(blocked, mp, merge, 1e-6)
    big = [(i * 37 + 11) % 256 for i in range(320 * 256 * 3)]
    full, _, _ = oracle.processor_patches(big, 256, 320, patch, temporal, merge)

    got = array.array("f")
    got.frombytes(open(sys.argv[3], "rb").read())
    at, worst_all = 0, 0.0
    for label, ref in (("patches", rows), ("embedding", emb), ("block 0", blocked), ("merger", merged), ("320x256 patches", full)):
        flat = [x for r in ref for x in r]
        mine = got[at:at + len(flat)]
        at += len(flat)
        worst = max(abs(a - b) / (1 + abs(b)) for a, b in zip(mine, flat))
        worst_all = max(worst_all, worst)
        print(f"  {label}: {len(flat)} values, largest |coax - oracle| / (1 + |oracle|) = {worst:.3g}")
    if at != len(got):
        sys.exit(f"output has {len(got)} values, expected {at}")
    if not worst_all <= TOLERANCE:
        sys.exit(f"outside the frozen tolerance {TOLERANCE}")
    print(f"within the frozen tolerance {TOLERANCE}")


def eval_count(shape):
    c = 1
    for s in shape:
        c *= s
    return c


if __name__ == "__main__":
    main()
