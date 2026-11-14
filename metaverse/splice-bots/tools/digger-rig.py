# Firm reason: runs inside Blender's Python (bpy), which the rig and the glTF export need.
# Rig the low-poly Digger, export the rigged GLB with idle and walk clips, and write the game's
# mesh table (metaverse/splice-bots/digger-mesh.coax).
#   Blender -b --factory-startup -P rig.py -- IN.glb OUT_RIGGED.glb OUT.coax
import bpy, sys, math, mathutils, numpy as np
argv = sys.argv[sys.argv.index("--") + 1:]
src, out_glb, out_coax = argv[0], argv[1], argv[2]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)
ob = [o for o in bpy.context.scene.objects if o.type == 'MESH'][0]
me = ob.data
me.calc_loop_triangles()
img = None
for n in me.materials[0].node_tree.nodes:
    if n.type == 'TEX_IMAGE': img = n.image
W, H = img.size
px = np.empty(W * H * 4, dtype=np.float32); img.pixels.foreach_get(px); px = px.reshape(H, W, 4)

# Model space in the game's axes: x right, y up, z ahead. Blender (bx, by, bz) -> game (by, bz, bx):
# a rotation (a cyclic swap), so the front (glTF +X, the nose/drill box) is ahead.
mw = ob.matrix_world
V = np.array([tuple(mw @ v.co) for v in me.vertices])
G = np.stack([V[:, 1], V[:, 2], V[:, 0]], axis=1)
cx = (G[:, 0].min() + G[:, 0].max()) / 2; cz = (G[:, 2].min() + G[:, 2].max()) / 2; y0 = G[:, 1].min()
G = G - np.array([cx, y0, cz])
HT = G[:, 1].max(); LEN = G[:, 2].max() - G[:, 2].min()
print("model", G.min(0), G.max(0))

# Bones (game model space, unscaled): name, parent, head, tail, capsule radius
def P(x, y, z): return np.array([x, y, z], dtype=float)
low = G[G[:, 1] < 0.24 * HT / 0.62]
bones = [("root", -1, P(0, 0, 0), P(0, 0.1, 0), 0),
         ("body", 0, P(0, 0.42, -0.26), P(0, 0.42, 0.26), 0.09),
         ("nose", 1, P(0, 0.42, 0.28), P(0, 0.40, 0.49), 0.10),
         ("sensor", 1, P(0, 0.44, -0.30), P(0, 0.48, -0.47), 0.10)]
legs = []
for name, sx, sz in (("fl", -1, 1), ("fr", 1, 1), ("hl", -1, -1), ("hr", 1, -1)):
    q = low[(np.sign(low[:, 0]) == sx) & (np.sign(low[:, 2]) == sz)]
    mid = q[(q[:, 1] > 0.13) & (q[:, 1] < 0.24)]
    foot = q[q[:, 1] < 0.04]
    kx, kz = mid[:, 0].mean(), mid[:, 2].mean()
    fx, fz = foot[:, 0].mean(), foot[:, 2].mean()
    hip = P(kx, 0.38, kz * 0.85 + sz * 0.02); knee = P(kx, 0.19, kz); toe = P(fx, 0.02, fz)
    legs.append((name, hip, knee, toe))
for name, hip, knee, toe in legs:
    bones.append(("thigh_" + name, 1, hip, knee, 0.035))
for i, (name, hip, knee, toe) in enumerate(legs):
    bones.append(("shin_" + name, 4 + i, knee, toe, 0.03))
for b in bones: print("BONE", b[0], b[1], np.round(b[2], 3), np.round(b[3], 3))

def seg_dist(p, a, b):
    ab = b - a; t = np.clip(((p - a) @ ab) / (ab @ ab), 0, 1)
    return np.linalg.norm(p - (a + t[:, None] * ab), axis=1)
D = np.stack([seg_dist(G, b[2], b[3]) - b[4] if i > 0 else np.full(len(G), 1e9) for i, b in enumerate(bones)], axis=1)
bone_of = D.argmin(1)
print("verts per bone", np.bincount(bone_of, minlength=len(bones)))

# Triangles: corners' positions, split normals, colour class from the bake at the centroid
PAL = ["yellow", "ink", "cream", "steel"]
def classify(c):
    r, g, b = c[:3]; mx, mn = max(r, g, b), min(r, g, b)
    if r > 0.35 and r - b > 0.22 and g > 0.15: return 0
    if mx < 0.22: return 1
    if mn > 0.45 and mx - mn < 0.2: return 2
    return 3 if mx > 0.30 else 1
uv = me.uv_layers.active.data
cn = me.corner_normals
keymap = {}; verts = []   # (pos idx, normal) welded
tris = []
for lt in me.loop_triangles:
    us = np.array([tuple(uv[l].uv) for l in lt.loops])
    cols = []
    for w in (np.array([1, 1, 1]) / 3, np.array([.6, .2, .2]), np.array([.2, .6, .2]), np.array([.2, .2, .6])):
        u, v = (w @ us)
        cols.append(px[min(H - 1, max(0, int(v * H))), min(W - 1, max(0, int((u % 1) * W)))])
    c = np.median(np.array(cols), axis=0)
    idx = []
    for k, l in enumerate(lt.loops):
        vi = lt.vertices[k]
        n = mw.to_3x3() @ cn[l].vector
        n = np.array([n.y, n.z, n.x]); n = n / (np.linalg.norm(n) or 1)
        key = (tuple(np.round(G[vi], 4)), tuple(np.round(n, 1)))
        if key not in keymap: keymap[key] = len(verts); verts.append((vi, n))
        idx.append(keymap[key])
    tris.append((classify(c), idx))
# Smooth the fills: the bake's tiny islands (a cream or steel speck on a black part) take the
# majority of their edge neighbours, so each part reads as one flat fill. Steel joins the black.
from collections import Counter, defaultdict
cls = [3 if c == 3 else c for c, _ in tris]
cls = [1 if c == 3 else c for c in cls]
edge = defaultdict(list)
for ti, (_, idx) in enumerate(tris):
    pk = [tuple(np.round(G[verts[i][0]], 4)) for i in idx]
    for a, b in ((0, 1), (1, 2), (2, 0)):
        edge[tuple(sorted((pk[a], pk[b])))].append(ti)
nb = defaultdict(set)
for ts_ in edge.values():
    for a in ts_:
        for b in ts_:
            if a != b: nb[a].add(b)
for it in range(4):
    new_ = []
    for ti in range(len(tris)):
        votes = Counter({cls[ti]: 1.5})
        for o in nb[ti]: votes[cls[o]] += 1
        new_.append(votes.most_common(1)[0][0])
    cls = new_
tris = [(cls[i], tris[i][1]) for i in range(len(tris))]
tris.sort(key=lambda t: t[0])
counts = [sum(1 for t in tris if t[0] == k) for k in range(4)]
print("verts", len(verts), "tris", len(tris), "per colour", counts)

# Encoding: base64 alphabet, a vertex as 9 chars (x, y, z as 12 bits each over +-SPAN, the normal
# octahedral as two 6-bit chars... 12 bits for both, the bone one char), a triangle as 9 chars
# (three 18-bit indices).
A = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
SPAN = 0.6   # model units: x and z within +-0.6, y within 0..1.2
def q12(x): return max(0, min(4095, int(round(x * 4095))))
def c2(v): return A[v >> 6] + A[v & 63]
def oct_enc(n):
    n = n / (abs(n).sum() or 1); x, y = n[0], n[2]
    if n[1] < 0: x, y = (1 - abs(y)) * np.sign(x or 1), (1 - abs(x)) * np.sign(y or 1)
    return q12(x * 0.5 + 0.5), q12(y * 0.5 + 0.5)
vs = []
for vi, n in verts:
    p = G[vi]
    ox, oy = oct_enc(n)
    vs.append(c2(q12(p[0] / (2 * SPAN) + 0.5)) + c2(q12(p[1] / (2 * SPAN))) + c2(q12(p[2] / (2 * SPAN) + 0.5)) + A[ox >> 6] + A[oy >> 6] + A[bone_of[vi]])
ts = []
for _, idx in tris:
    ts.append("".join(A[(i >> 12) & 63] + A[(i >> 6) & 63] + A[i & 63] for i in idx))
vstr = "".join(vs); tstr = "".join(ts)

# Rig in Blender (Blender axes: game (x, y, z) = blender (by, bz, bx), so blender = (gz, gx, gy) after
# the centring) and export the rigged GLB with two clips.
def to_b(p): return mathutils.Vector((p[2] + cz, p[0] + cx, p[1] + y0))
arm = bpy.data.armatures.new("DiggerRig"); ao = bpy.data.objects.new("DiggerRig", arm)
bpy.context.scene.collection.objects.link(ao); bpy.context.view_layer.objects.active = ao
bpy.ops.object.mode_set(mode='EDIT')
eb = []
for name, parent, h, t, _ in bones:
    b = arm.edit_bones.new(name); b.head = to_b(h); b.tail = to_b(t)
    if parent >= 0: b.parent = eb[parent]
    eb.append(b)
bpy.ops.object.mode_set(mode='OBJECT')
for name, *_ in bones: ob.vertex_groups.new(name=name)
for vi in range(len(me.vertices)):
    ob.vertex_groups[bones[bone_of[vi]][0]].add([vi], 1.0, 'REPLACE')
ob.parent = ao; mod = ob.modifiers.new("Rig", 'ARMATURE'); mod.object = ao
ao.animation_data_create()
def clip(name, frames, pose):
    act = bpy.data.actions.new(name); ao.animation_data.action = act
    for f in range(0, frames + 1, 2):
        ph = 2 * math.pi * f / frames
        for bn, rot, loc in pose(ph):
            pb = ao.pose.bones[bn]; pb.rotation_mode = 'XYZ'
            pb.rotation_euler = rot; pb.location = loc
            pb.keyframe_insert("rotation_euler", frame=f); pb.keyframe_insert("location", frame=f)
    act.use_fake_user = True
    tr = ao.animation_data.nla_tracks.new(); tr.name = name; tr.strips.new(name, 0, act)
    ao.animation_data.action = None
def idle(ph):
    return [("body", (0, 0, 0), (0, 0.01 * math.sin(2 * ph), 0)), ("sensor", (0, 0.35 * math.sin(ph), 0), (0, 0, 0)),
            ("nose", (0.05 * math.sin(2 * ph), 0, 0), (0, 0, 0))]
def walk(ph):
    out = [("body", (0, 0, 0), (0, 0.012 * math.sin(2 * ph), 0))]
    for i, name in enumerate(("fl", "fr", "hl", "hr")):
        o = 0 if name in ("fl", "hr") else math.pi
        out.append(("thigh_" + name, (0.45 * math.sin(ph + o), 0, 0), (0, 0, 0)))
        out.append(("shin_" + name, (-0.35 * max(0, math.sin(ph + o + math.pi / 2)), 0, 0), (0, 0, 0)))
    return out
clip("idle", 48, idle); clip("walk", 24, walk)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.export_scene.gltf(filepath=out_glb, export_format='GLB', export_animations=True, export_animation_mode='NLA_TRACKS',
                          export_skins=True, export_image_format='AUTO')

# The coax table.
def pt(p): return "v3(%.4f, %.4f, %.4f)" % tuple(p)
with open(out_coax, "w") as f:
    f.write("""// The Digger's mesh (GitHub #13505): the designer's Tripo model, decimated to %d triangles and baked to one
// flat base colour in Blender, each triangle given the nearest of four cel fills (yellow, ink, cream,
// steel) so the game's cel pass and ink overlay light it as they light the kit. Written by
// metaverse/splice-bots/tools/digger-rig.py from metaverse/splice-bots/assets/digger-rigged.glb's
// source; do not edit by hand. Model units: 1 = %.3f m long, the nose (the drill box) ahead (+z).
// A vertex is 9 characters of the base64 alphabet: x, y, z as two each (12 bits over DGM_SPAN), the
// normal as two (octahedral, 6 bits each) and its bone as one. A triangle is 9: three 18-bit indices.
// The tables are in pieces of DGM_PIECE records (dgm_v, dgm_t). The triangles are sorted by fill: DGM_FILL_END[k] is where fill k ends.
DGM_SPAN :: %.3f;
DGM_VERTS :: %d; DGM_TRIS :: %d;
DGM_FILL_END :: [4] int.[%d, %d, %d, %d];
DGM_BONES :: %d;
// Each bone: its parent (-1: none), its pivot (where it turns) in model units.
DGM_PARENT :: [DGM_BONES] int.[%s];
DGM_PIVOT :: (b: int) -> Point {
  pivots := [DGM_BONES] Point.[%s];
  return pivots[b];
}
""" % (len(tris), LEN, SPAN, len(verts), len(tris), counts[0], counts[0] + counts[1], counts[0] + counts[1] + counts[2], len(tris), len(bones),
       ", ".join(str(b[1]) for b in bones), ", ".join(pt(b[2]) for b in bones)))
    # The tables in pieces of DGM_PIECE records, one line of C each (a line the exporter reads is at most 64 KB).
    f.write("DGM_PIECE :: 900;   // records in one piece of a table\n")
    for name, text in (("dgm_v", vstr), ("dgm_t", tstr)):
        f.write("%s :: (k: int) -> string {\n" % name)
        step = 9 * 900
        for k in range(0, len(text), step):
            f.write("  if k == %d { return \"%s\"; }\n" % (k // step, text[k:k + step]))
        f.write("  return \"\";\n}\n")
print("LEN", LEN, "HT", HT, "bytes", len(vstr) + len(tstr))
