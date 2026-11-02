"""Independent check of chapter 144's original-weight C-RADIOv3-B slices.

    python3 -I psionic/tests/radio_slices.py CONFIG.json SAFETENSORS OUT.f32

Reads the safetensors header itself (header only, no hashing), checks that
the tensors are exactly those the config and the reference's model
definition imply, reads the tensors from the file, recomputes everything with
radio_oracle.py's binary64 functions and compares Coax's f32 outputs: the 64
by 48 image's patches, embedding, block 0, and its summary and spatial
features after all 12 blocks; the 48 by 64 image's embedding; the 224 by 224
image's patches. Tolerance, frozen before the first comparison:
|coax - oracle| <= 1e-4 * (1 + |oracle|) for every value.
"""
import array
import importlib.util
import json
import os
import struct
import sys

TOLERANCE = 1e-4
here = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("radio_oracle", os.path.join(here, "radio_oracle.py"))
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)

# vit_base_patch16_v2_224 in the reference's extra_timm_models.py: width
# 768, 12 blocks of 12 heads, timm's MLP ratio 4, reg_tokens 4.
WIDTH, DEPTH, HEADS, INNER, TIMM_REGISTERS = 768, 12, 12, 3072, 4


def expected(cfg):
    a = cfg["args"]
    p, side = cfg["patch_size"], a["cpe_max_size"] // cfg["patch_size"]
    cls = len({t["name"] for t in a["teachers"]}) if a["cls_token_per_teacher"] else 1
    summary = [i for i, t in enumerate(a["teachers"]) if t.get("use_summary", True)]
    e = {"summary_idxs": ("I64", [len(summary)]), "input_conditioner.norm_mean": ("F32", [3, 1, 1]),
         "input_conditioner.norm_std": ("F32", [3, 1, 1]),
         "model.patch_generator.embedder.weight": ("F32", [WIDTH, 3 * p * p]),
         "model.patch_generator.pos_embed": ("F32", [1, side * side, WIDTH]),
         "model.patch_generator.cls_token.token": ("F32", [cls + a["cpe_num_registers"], WIDTH]),
         "model.reg_token": ("F32", [1, TIMM_REGISTERS, WIDTH])}
    for b in range(DEPTH):
        for name, shape in (("norm1.weight", [WIDTH]), ("norm1.bias", [WIDTH]), ("attn.qkv.weight", [3 * WIDTH, WIDTH]),
                            ("attn.qkv.bias", [3 * WIDTH]), ("attn.proj.weight", [WIDTH, WIDTH]), ("attn.proj.bias", [WIDTH]),
                            ("ls1.grandma", [WIDTH]), ("norm2.weight", [WIDTH]), ("norm2.bias", [WIDTH]),
                            ("mlp.fc1.weight", [INNER, WIDTH]), ("mlp.fc1.bias", [INNER]), ("mlp.fc2.weight", [WIDTH, INNER]),
                            ("mlp.fc2.bias", [WIDTH]), ("ls2.grandma", [WIDTH])):
            e[f"model.blocks.{b}.{name}"] = ("F32", shape)
    return {"radio_model." + n: s for n, s in e.items()}, p, side, cls, a["cpe_num_registers"], summary


def main():
    cfg = json.load(open(sys.argv[1]))
    path = sys.argv[2]
    want, patch, side, cls, registers, summary = expected(cfg)
    f = open(path, "rb")
    n = struct.unpack("<Q", f.read(8))[0]
    header = json.loads(f.read(n))
    base = 8 + n
    tensors = {k: v for k, v in header.items() if k != "__metadata__"}
    wrong = [k for k, (dt, sh) in want.items() if k not in tensors or tensors[k]["dtype"] != dt or tensors[k]["shape"] != sh]
    extra = [k for k in tensors if k not in want]
    count = sum(eval_size(v["shape"]) for v in tensors.values())
    print(f"oracle inventory: {len(tensors)} tensors, {len(want)} expected, {len(wrong)} wrong, {len(extra)} extra, {count} elements")

    def read(name):
        a, b = tensors[name]["data_offsets"]
        f.seek(base + a)
        raw = f.read(b - a)
        if tensors[name]["dtype"] == "I64":
            return list(struct.unpack(f"<{(b - a) // 8}q", raw))
        out = array.array("f")
        out.frombytes(raw)
        if sys.byteorder != "little":
            out.byteswap()
        return out

    pre = "radio_model."
    idxs = read(pre + "summary_idxs")
    mean, std = list(read(pre + "input_conditioner.norm_mean")), list(read(pre + "input_conditioner.norm_std"))
    print(f"oracle buffers: summary_idxs {idxs} (config {summary}), mean {mean}, std {std}")
    ok = not wrong and not extra and idxs == summary
    w = read(pre + "model.patch_generator.embedder.weight")
    table = read(pre + "model.patch_generator.pos_embed")
    tokens = read(pre + "model.patch_generator.cls_token.token")
    blocks = []
    for b in range(DEPTH):
        blocks.append({part: read(f"{pre}model.blocks.{b}.{name}") for part, name in zip(oracle.BLOCK_PARTS, (
            "norm1.weight", "norm1.bias", "attn.qkv.weight", "attn.qkv.bias", "attn.proj.weight", "attn.proj.bias", "ls1.grandma",
            "norm2.weight", "norm2.bias", "mlp.fc1.weight", "mlp.fc1.bias", "mlp.fc2.weight", "mlp.fc2.bias", "ls2.grandma"))})

    def image(h, wd):
        px = [(i * 37 + 11) % 256 for i in range(3 * h * wd)]
        return oracle.im2patches(oracle.conditioned(px, h, wd, mean, std), patch), h // patch, wd // patch

    sections = []
    patches, rows, cols = image(48, 64)
    sections.append(("patches 64x48", patches))
    y = oracle.embed(patches, w, table, side, WIDTH, rows, cols, tokens)
    sections.append(("embedding 64x48", y))
    for b in range(DEPTH):
        y = oracle.block(y, blocks[b], HEADS, INNER, 1e-6)
        if b == 0:
            sections.append(("block 0", y))
    s, feats = oracle.outputs(y, cls, summary, cls + registers)
    sections.append(("summary (12 blocks)", [s]))
    sections.append(("spatial features (12 blocks)", feats))
    tall, rows, cols = image(64, 48)
    sections.append(("embedding 48x64", oracle.embed(tall, w, table, side, WIDTH, rows, cols, tokens)))
    large, _, _ = image(224, 224)
    sections.append(("patches 224x224", large))

    coax = array.array("f")
    with open(sys.argv[3], "rb") as g:
        coax.frombytes(g.read())
    at = 0
    for name, rows_ in sections:
        flat = [v for r in rows_ for v in r]
        worst = max(abs(c - o) / (1 + abs(o)) for c, o in zip(coax[at:at + len(flat)], flat))
        print(f"  {name}: {len(flat)} values, largest |coax - oracle| / (1 + |oracle|) = {worst:.2e}")
        ok = ok and worst <= TOLERANCE
        at += len(flat)
    ok = ok and at == len(coax)
    print(f"summary {len(s)} values, spatial features {len(feats)} by {len(feats[0])}")
    print(f"within the frozen tolerance {TOLERANCE}" if ok else "OUTSIDE the frozen tolerance or inventory mismatch")
    sys.exit(0 if ok else 1)


def eval_size(shape):
    n = 1
    for d in shape:
        n *= d
    return n


if __name__ == "__main__":
    main()
