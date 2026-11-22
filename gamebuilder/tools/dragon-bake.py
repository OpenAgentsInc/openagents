#!/usr/bin/env python3
# Bakes the Shapechange dragon (assets/metaverse/beasts/dragon.gltf, CC0 1.0) into web/ghost-data.coax,
# the bytes web/ghost.coax reads, so the web world's player is the dragon. Offline; the page never runs it.
#
#   python3 gamebuilder/tools/dragon-bake.py [SCALE]        (from the repo root; SCALE 0.3 when not given)
#
# Layout (little endian), as tools/ghost-bake.coax's GBG1 but "GBD1", 20-byte vertices with a colour:
#   "GBD1", then u32s: vertices, triangles, joints, clips, samples a second, moved joints
#   a joint: its parent (u8, 255 none); its rest translation (3 f32); its inverse bind, 3 rows of 4 (12 f32)
#   a moved joint: its number (u8)
#   a clip: samples (u32), seconds (f32), metres a loop (f32)
#   a vertex, 20 bytes: x y z (i16, 1/8192 m), normal (3 i8), four joints (u8), four weights (u8), colour (3 u8, display)
#   a triangle: three vertices (u16)
#   a clip's samples: each joint's rotation x y z w (4 i16, 32767ths), then each moved joint's translation (3 i16)
# Metres, +y up, +z the way it faces, its left +x.
import base64, json, math, os, struct, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SRC = os.path.join(ROOT, "assets/metaverse/beasts/dragon.gltf")
OUT = os.path.join(ROOT, "gamebuilder/web/ghost-data.coax")
SCALE = float(sys.argv[1]) if len(sys.argv) > 1 else 0.3
FPS = 24
# ghost.coax's clips in order: idle, run (the walk), air (the glide), land, breath, then flight's:
# the wing beat, and the banks into a turn
CLIPS = ["idle", "walk", "glide", "land", "breath", "fly", "bank_left", "bank_right"]
POS = 8192.0

g = json.load(open(SRC))
blob = open(os.path.join(os.path.dirname(SRC), g["buffers"][0]["uri"]), "rb").read()

def acc(i):
    a = g["accessors"][i]; v = g["bufferViews"][a["bufferView"]]
    comp = {5120: "b", 5121: "B", 5122: "h", 5123: "H", 5125: "I", 5126: "f"}[a["componentType"]]
    n = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[a["type"]]
    size = struct.calcsize("<" + comp); stride = v.get("byteStride", size * n)
    off = v.get("byteOffset", 0) + a.get("byteOffset", 0)
    out = []
    for k in range(a["count"]):
        t = struct.unpack_from("<" + comp * n, blob, off + k * stride)
        if a.get("normalized"):
            d = {5121: 255.0, 5123: 65535.0, 5120: 127.0, 5122: 32767.0}[a["componentType"]]
            t = tuple(x / d for x in t)
        out.append(t)
    return out

skin = g["skins"][0]
joints = skin["joints"]
J = len(joints)
node_parent = {}
for i, n in enumerate(g["nodes"]):
    for c in n.get("children", []): node_parent[c] = i
jindex = {nd: k for k, nd in enumerate(joints)}
parent = []
for k, nd in enumerate(joints):
    p = node_parent.get(nd)
    pk = jindex.get(p, -1) if p is not None else -1
    if pk >= k: sys.exit("joint order: a parent after its child")
    parent.append(pk)
# the rig's own node must be still (no turn or scale): the joints' rest is then their world
root_p = node_parent.get(joints[0])
if root_p is not None:
    rn = g["nodes"][root_p]
    if rn.get("rotation", [0, 0, 0, 1]) != [0, 0, 0, 1] or rn.get("scale", [1, 1, 1]) != [1, 1, 1] or any(abs(x) > 1e-6 for x in rn.get("translation", [0, 0, 0])):
        sys.exit("the rig's node moves: not handled")

rest_t = [[x * SCALE for x in g["nodes"][nd].get("translation", [0, 0, 0])] for nd in joints]
rest_r = [g["nodes"][nd].get("rotation", [0, 0, 0, 1]) for nd in joints]
ibm = acc(skin["inverseBindMatrices"])
bind = []
for m in ibm:  # column major
    rows = []
    for r in range(3):
        rows += [m[0 * 4 + r], m[1 * 4 + r], m[2 * 4 + r], m[3 * 4 + r] * SCALE]
    bind.append(rows)

# the mesh, each primitive's material colour on its vertices
def srgb(c):
    c = max(0.0, min(1.0, c))
    return 12.92 * c if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055
verts = []; tris = []
for pr in g["meshes"][0]["primitives"]:
    at = pr["attributes"]; mat = g["materials"][pr.get("material", 0)]
    base = mat.get("pbrMetallicRoughness", {}).get("baseColorFactor", [1, 1, 1, 1])
    glow = mat.get("emissiveFactor", [0, 0, 0])
    col = tuple(int(round(min(1.0, srgb(base[k]) + glow[k] * 0.5) * 255)) for k in range(3))
    P = acc(at["POSITION"]); N = acc(at["NORMAL"]); JJ = acc(at["JOINTS_0"]); W = acc(at["WEIGHTS_0"])
    first = len(verts)
    for k in range(len(P)):
        w = [max(0.0, x) for x in W[k]]; s = sum(w) or 1.0
        wb = [int(round(x / s * 255)) for x in w]
        wb[wb.index(max(wb))] += 255 - sum(wb)
        verts.append((P[k], N[k], JJ[k], wb, col))
    idx = [t[0] for t in acc(pr["indices"])] if "indices" in pr else list(range(len(P)))
    for k in range(0, len(idx), 3): tris.append((first + idx[k], first + idx[k + 1], first + idx[k + 2]))
if len(verts) > 65535: sys.exit("too many vertices")

# the clips, sampled
anims = {a["name"]: a for a in g["animations"]}
def track(a, nd, path):
    for ch in a["channels"]:
        if ch["target"].get("node") == nd and ch["target"]["path"] == path:
            s = a["samplers"][ch["sampler"]]
            return [t[0] for t in acc(s["input"])], acc(s["output"]), s.get("interpolation", "LINEAR")
    return None
def nlerp_q(a, b, f):
    d = sum(x * y for x, y in zip(a, b))
    if d < 0: b = [-x for x in b]; d = -d
    if d > 0.9995:
        q = [x + (y - x) * f for x, y in zip(a, b)]
    else:
        th = math.acos(d); s = math.sin(th)
        q = [(math.sin((1 - f) * th) * x + math.sin(f * th) * y) / s for x, y in zip(a, b)]
    l = math.sqrt(sum(x * x for x in q)) or 1.0
    return [x / l for x in q]
def sample(tr, t, quat):
    times, vals, interp = tr
    if t <= times[0]: return list(vals[0])
    if t >= times[-1]: return list(vals[-1])
    for i in range(len(times) - 1):
        if times[i] <= t <= times[i + 1]:
            if interp == "STEP": return list(vals[i])
            f = (t - times[i]) / (times[i + 1] - times[i])
            return nlerp_q(vals[i], vals[i + 1], f) if quat else [x + (y - x) * f for x, y in zip(vals[i], vals[i + 1])]
    return list(vals[-1])

moved = set()
for name in CLIPS:
    a = anims[name]
    for k, nd in enumerate(joints):
        tr = track(a, nd, "translation")
        if tr and any(any(abs(v[c] - rest_t[k][c] / SCALE) > 1e-4 for c in range(3)) for v in tr[1]): moved.add(k)
moved = sorted(moved)

clip_head = []; clip_bytes = bytearray()
for name in CLIPS:
    a = anims[name]
    tracks_r = [track(a, nd, "rotation") for nd in joints]
    tracks_t = [track(a, nd, "translation") for nd in joints]
    allt = [t for tr in tracks_r + tracks_t if tr for t in tr[0]]
    t0, t1 = min(allt), max(allt)
    secs = max(t1 - t0, 1.0 / FPS)
    frames = max(2, int(round(secs * FPS)) + 1)
    for f in range(frames):
        t = t0 + secs * f / (frames - 1)
        for k in range(J):
            q = sample(tracks_r[k], t, True) if tracks_r[k] else rest_r[k]
            clip_bytes += struct.pack("<4h", *[max(-32767, min(32767, int(round(x * 32767)))) for x in q])
        for k in moved:
            v = sample(tracks_t[k], t, False) if tracks_t[k] else [x / SCALE for x in rest_t[k]]
            clip_bytes += struct.pack("<3h", *[max(-32767, min(32767, int(round(x * SCALE * POS)))) for x in v])
    clip_head.append((frames, secs, 0.0))

out = bytearray(b"GBD1")
out += struct.pack("<6I", len(verts), len(tris), J, len(CLIPS), FPS, len(moved))
out += bytes(255 if p < 0 else p for p in parent)
for t in rest_t: out += struct.pack("<3f", *t)
for b in bind: out += struct.pack("<12f", *b)
out += bytes(moved)
for fr, s, m in clip_head: out += struct.pack("<Iff", fr, s, m)
for p, n, jj, wb, col in verts:
    out += struct.pack("<3h", *[max(-32767, min(32767, int(round(x * SCALE * POS)))) for x in p])
    out += struct.pack("<3b", *[max(-127, min(127, int(round(x * 127)))) for x in n])
    out += bytes(jj) + bytes(wb) + bytes(col)
for t in tris: out += struct.pack("<3H", *t)
out += clip_bytes

text = base64.b64encode(bytes(out)).decode().rstrip("=")   # ghost.coax's reader takes no padding
CH = 4068
chunks = [text[i:i + CH] for i in range(0, len(text), CH)]
with open(OUT, "w") as f:
    f.write("// Generated by tools/dragon-bake.py; do not edit. The web world's player as the Shapechange dragon\n")
    f.write("// (assets/metaverse/beasts/dragon.gltf, CC0 1.0), scaled by %g, with five of its clips (%s), as base-64 text.\n" % (SCALE, ", ".join(CLIPS)))
    f.write("// web/ghost.coax reads it; tools/dragon-bake.py has the layout.\n")
    f.write("GHOST_DATA_BYTES :: %d;\nGHOST_DATA_CHUNKS :: %d;\n" % (len(out), len(chunks)))
    f.write("ghost_data_chunk :: (i: int) -> string {\n")
    for i, c in enumerate(chunks): f.write('  if i == %d { return "%s"; }\n' % (i, c))
    f.write('  return "";\n}\n')
print("dragon: %d vertices, %d triangles, %d joints (%d moved), clips %s, %d bytes, %d chunks" % (
    len(verts), len(tris), J, len(moved), [(n, c[0], round(c[1], 3)) for n, c in zip(CLIPS, clip_head)], len(out), len(chunks)))
