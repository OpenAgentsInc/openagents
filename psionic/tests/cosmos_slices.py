"""Chapter 166's tooling: the Cosmos3-Edge manifest and its real-weight slices.

Reads safetensors headers only (never unpickles, never runs model code), and
the bytes of a few small tensors from the two files small enough to keep
locally (transformer shard 2 and the VAE). Writes:

  psionic/fixtures/cosmos3-edge-manifest.txt  every weight file's size and
      sha256, and every tensor group's count, dtype and shapes
  psionic/tests/cosmos-slices.coax            the bf16 bits of four slices,
      their header shapes, and float64 reference outputs

Usage, with DIR holding the downloaded files and the two remote headers:

  python3 -I psionic/tests/cosmos_slices.py DIR

DIR/tree.json is the Hugging Face file listing at the pinned revision;
DIR/hdr_<path with / as _>.json are the range-read headers of the two files
not kept locally. Python is tooling only: nothing here ships.
"""
import json, re, struct, sys, collections, os

REV = "344d602b128d1bbdacb43b08d0a3626f46343e29"
D = sys.argv[1]
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
WEIGHTS = [
    ("transformer/diffusion_pytorch_model-00001-of-00002.safetensors", None),
    ("transformer/diffusion_pytorch_model-00002-of-00002.safetensors", "transformer/shard2.safetensors"),
    ("vae/diffusion_pytorch_model.safetensors", "vae/vae.safetensors"),
    ("vision_encoder/model.safetensors", None),
]


def header(path, local):
    if local:
        f = open(os.path.join(D, local), "rb")
        n = struct.unpack("<Q", f.read(8))[0]
        return json.loads(f.read(n)), n
    b = open(os.path.join(D, "hdr_" + path.replace("/", "_") + ".json"), "rb").read()
    return json.loads(b), len(b)


def bits(local, h, n, name):
    """The raw bf16 words of one tensor, read at its offsets."""
    e = h[name]
    assert e["dtype"] == "BF16"
    a, b = e["data_offsets"]
    f = open(os.path.join(D, local), "rb")
    f.seek(8 + n + a)
    raw = f.read(b - a)
    return list(struct.unpack("<%dH" % ((b - a) // 2), raw)), e["shape"]


def bf16(w):
    return struct.unpack("<f", struct.pack("<I", w << 16))[0]


tree = {e["path"]: e for e in json.load(open(os.path.join(D, "tree.json"))) if e["type"] == "file"}
heads = {}
lines = ["# Cosmos3-Edge, nvidia/Cosmos3-Edge at revision " + REV,
         "# Written by psionic/tests/cosmos_slices.py from safetensors headers only.",
         "# file  bytes  sha256  tensors  header-bytes"]
for path, local in WEIGHTS:
    h, n = header(path, local)
    h.pop("__metadata__", None)
    heads[path] = (h, n, local)
    t = tree[path]
    lines.append("file %s %d %s %d %d" % (path, t["size"], t["lfs"]["oid"], len(h), n))
lines.append("# group  file  count  dtype  shapes (layer index as N)")
total = 0
for path, (h, n, local) in heads.items():
    groups = collections.OrderedDict()
    for name in sorted(h):
        e = h[name]
        k = 1
        for s in e["shape"]:
            k *= s
        total += k
        g = re.sub(r"\.\d+\.", ".N.", name)
        groups.setdefault(g, []).append(e)
    for g, es in groups.items():
        shapes = sorted(set("x".join(str(s) for s in e["shape"]) for e in es))
        dt = sorted(set(e["dtype"] for e in es))
        lines.append("group %s %s %d %s %s" % (g, path.split("/")[0], len(es), ",".join(dt), " ".join(shapes)))
lines.append("parameters %d" % total)
open(os.path.join(ROOT, "psionic/fixtures/cosmos3-edge-manifest.txt"), "w").write("\n".join(lines) + "\n")

# Slices. Inputs are fixed formulas the Coax side repeats.
sh2 = heads["transformer/diffusion_pytorch_model-00002-of-00002.safetensors"]
vae = heads["vae/diffusion_pytorch_model.safetensors"]
pin_w, pin_s = bits(sh2[2], sh2[0], sh2[1], "proj_in.weight")
pin_b, _ = bits(sh2[2], sh2[0], sh2[1], "proj_in.bias")
pout_w, pout_s = bits(sh2[2], sh2[0], sh2[1], "proj_out.weight")
pout_b, _ = bits(sh2[2], sh2[0], sh2[1], "proj_out.bias")
cin_w, cin_s = bits(vae[2], vae[0], vae[1], "encoder.conv_in.weight")
cin_b, _ = bits(vae[2], vae[0], vae[1], "encoder.conv_in.bias")
ROWS_IN, ROWS_OUT, FILTERS = 8, 2, 2


def u_in(i): return ((i * 37) % 17 - 8) / 8.0
def u_out(j): return ((j * 29) % 23 - 11) / 11.0
def pixel(c, y, x): return (((c * 16 + y * 4 + x) * 7) % 11 - 5) / 5.0


ref_in = []
for r in range(ROWS_IN):
    s = 0.0
    for i in range(192):
        s += bf16(pin_w[r * 192 + i]) * u_in(i)
    ref_in.append(bf16(pin_b[r]) + s)
ref_out = []
for r in range(ROWS_OUT):
    s = 0.0
    for j in range(2048):
        s += bf16(pout_w[r * 2048 + j]) * u_out(j)
    ref_out.append(bf16(pout_b[r]) + s)
# A 3 x 4 x 4 image, patchified the Wan way (channel c*4 + column-offset*2 +
# row-offset), is 12 x 2 x 2. One frame: the causal pad puts two zero frames
# first, so only temporal tap 2 sees it. Output at every position of the 2 x 2
# grid, zero padding of 1 around it.
patched = [[[0.0] * 2 for _ in range(2)] for _ in range(12)]
for c in range(3):
    for y in range(4):
        for x in range(4):
            patched[c * 4 + (x % 2) * 2 + (y % 2)][y // 2][x // 2] = pixel(c, y, x)
ref_conv = []
for o in range(FILTERS):
    for y in range(2):
        for x in range(2):
            s = 0.0
            for c in range(12):
                for dy in range(3):
                    for dx in range(3):
                        yy, xx = y + dy - 1, x + dx - 1
                        if 0 <= yy < 2 and 0 <= xx < 2:
                            s += bf16(cin_w[(((o * 12 + c) * 3 + 2) * 3 + dy) * 3 + dx]) * patched[c][yy][xx]
            ref_conv.append(bf16(cin_b[o]) + s)


def words(name, ws):
    out = ["%s :: [%d]u16.[" % (name, len(ws))]
    for i in range(0, len(ws), 16):
        out.append("    " + ",".join("0x%04x" % w for w in ws[i:i + 16]) + ("," if i + 16 < len(ws) else ""))
    out.append("];")
    return out


def f64s(name, vs):
    return ["%s :: [%d]f64.[%s];" % (name, len(vs), ",".join(repr(v) for v in vs))]


conv_last = [cin_w[(((o * 12 + c) * 3 + 2) * 3 + dy) * 3 + dx]
             for o in range(FILTERS) for c in range(12) for dy in range(3) for dx in range(3)]
src = ["// Generated by psionic/tests/cosmos_slices.py from nvidia/Cosmos3-Edge at",
       "// revision " + REV + ". Do not edit.",
       "// Header shapes of the tensors chapter 166 checks against config.json,",
       "// the bf16 words of four slices, and float64 references computed from them.",
       "CS_HEADER_SHAPES :: [%d]int.[%s];" % (
           0, ""),
       ]
names = [("transformer/diffusion_pytorch_model-00001-of-00002.safetensors", "layers.0.self_attn.to_q.weight"),
         ("transformer/diffusion_pytorch_model-00001-of-00002.safetensors", "layers.0.self_attn.to_k.weight"),
         ("transformer/diffusion_pytorch_model-00001-of-00002.safetensors", "layers.0.self_attn.add_q_proj.weight"),
         ("transformer/diffusion_pytorch_model-00001-of-00002.safetensors", "layers.0.self_attn.add_k_proj.weight"),
         ("transformer/diffusion_pytorch_model-00001-of-00002.safetensors", "layers.0.self_attn.norm_added_q.weight"),
         ("transformer/diffusion_pytorch_model-00001-of-00002.safetensors", "layers.0.mlp.up_proj.weight"),
         ("transformer/diffusion_pytorch_model-00001-of-00002.safetensors", "layers.0.mlp_moe_gen.down_proj.weight"),
         ("transformer/diffusion_pytorch_model-00001-of-00002.safetensors", "embed_tokens.weight"),
         ("transformer/diffusion_pytorch_model-00002-of-00002.safetensors", "proj_in.weight"),
         ("transformer/diffusion_pytorch_model-00002-of-00002.safetensors", "proj_out.weight"),
         ("transformer/diffusion_pytorch_model-00002-of-00002.safetensors", "time_embedder.linear_1.weight"),
         ("transformer/diffusion_pytorch_model-00002-of-00002.safetensors", "action_proj_out.bias.weight"),
         ("vae/diffusion_pytorch_model.safetensors", "encoder.conv_in.weight"),
         ("vae/diffusion_pytorch_model.safetensors", "encoder.conv_out.weight"),
         ("vae/diffusion_pytorch_model.safetensors", "decoder.conv_in.weight"),
         ("vae/diffusion_pytorch_model.safetensors", "decoder.conv_out.weight"),
         ("vae/diffusion_pytorch_model.safetensors", "post_quant_conv.weight"),
         ("vision_encoder/model.safetensors", "model.visual.embeddings.patch_embedding.weight"),
         ("vision_encoder/model.safetensors", "model.projector.linear_fc1.weight")]
flat = []
for p, n in names:
    s = list(heads[p][0][n]["shape"]) + [1] * (5 - len(heads[p][0][n]["shape"]))
    flat += s
src[-1] = "CS_HEADER_SHAPES :: [%d]int.[%s];" % (len(flat), ",".join(str(v) for v in flat))
layer_count = {}
for p, (h, n, local) in heads.items():
    for name in h:
        m = re.match(r"layers\.(\d+)\.", name)
        if m and p.startswith("transformer"):
            layer_count[int(m.group(1))] = layer_count.get(int(m.group(1)), 0) + 1
src.append("CS_HEADER_LAYERS :: %d;          // distinct transformer layer indices" % len(layer_count))
src.append("CS_HEADER_PER_LAYER :: %d;       // tensors in every layer" % min(layer_count.values()))
assert min(layer_count.values()) == max(layer_count.values())
src.append("CS_HEADER_TENSORS :: %d;       // tensors in all four weight files" % sum(len(h[0]) for h in heads.values()))
src.append("CS_HEADER_PARAMETERS :: %d;" % total)
src += words("CS_PROJ_IN_W", pin_w[:ROWS_IN * 192])
src += words("CS_PROJ_IN_B", pin_b[:ROWS_IN])
src += words("CS_PROJ_OUT_W", pout_w[:ROWS_OUT * 2048])
src += words("CS_PROJ_OUT_B", pout_b[:ROWS_OUT])
src += words("CS_CONV_IN_W", conv_last)
src += words("CS_CONV_IN_B", cin_b[:FILTERS])
src += f64s("CS_REF_PROJ_IN", ref_in)
src += f64s("CS_REF_PROJ_OUT", ref_out)
src += f64s("CS_REF_CONV_IN", ref_conv)
open(os.path.join(ROOT, "psionic/tests/cosmos-slices.coax"), "w").write("\n".join(src) + "\n")
print("tensors", sum(len(h[0]) for h in heads.values()), "parameters", total, "layers", len(layer_count))
print("proj_in", ref_in[:3], "proj_out", ref_out, "conv_in", ref_conv[:4])
