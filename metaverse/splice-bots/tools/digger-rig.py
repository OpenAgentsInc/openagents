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

# Encoding (GitHub #13505, the 30k Digger): one bit stream a fill, read in order as the game draws.
# Each fill's triangles are its body's, then its legs' (a triangle with a corner on a leg bone), each
# part ordered by walking across shared edges so neighbours come close together. A corner is:
#   1, then d - 1 (Exp-Golomb, DGM_KD): the vertex the corner d back in this fill wrote, copied as is;
#   0 1, then d - 1: a new vertex at the place (and bone) of the corner d back, with its own normal;
#   0 0, then x, y, z (12 bits each over +-SPAN) as zigzagged Exp-Golomb (DGM_KP) differences from the
#        triangle's last corner (or the fill's last new vertex), the bone (1: the same as that one's,
#        0 and 4 bits), then the normal.
# A normal is octahedral, DGM_NB bits a component. The game keeps the places of the last DGM_RING
# corners (a corner copied from further back has none). Thirteen bits a pair of characters, the high
# bit first: the pair's value is its first digit times 93 plus its second, a digit being a printable
# character from space to '~' without the quote and the backslash (ALPHA, in order). Each fill starts
# on a pair (#13552: 7.7% fewer characters than the six bits a character before).
SPAN = 0.6   # model units: x and z within +-0.6, y within 0..1.2
NB = 7; NM = (1 << NB) - 1; KD = 3; KP = 3; RING = 4096
def q12(x): return max(0, min(4095, int(round(x * 4095))))
def oct_q(n):
    n = n / (abs(n).sum() or 1); x, y = n[0], n[2]
    if n[1] < 0: x, y = (1 - abs(y)) * (1 if x >= 0 else -1), (1 - abs(x)) * (1 if y >= 0 else -1)
    return (int(round((x * 0.5 + 0.5) * NM)), int(round((y * 0.5 + 0.5) * NM)))
def vkey(i):
    vi, n = verts[i]; p = G[vi]
    return ((q12(p[0] / (2 * SPAN) + 0.5), q12(p[1] / (2 * SPAN)), q12(p[2] / (2 * SPAN) + 0.5)), oct_q(n), int(bone_of[vi]))
def walk_order(ts):
    edge = defaultdict(list)
    for i, t in enumerate(ts):
        for a, b in ((0, 1), (1, 2), (2, 0)): edge[tuple(sorted((t[a][0], t[b][0])))].append(i)
    nbr = defaultdict(list)
    for l in edge.values():
        for a in l:
            for b in l:
                if a != b: nbr[a].append(b)
    seen = [False] * len(ts); out = []
    for s in range(len(ts)):
        if seen[s]: continue
        stack = [s]
        while stack:
            i = stack[-1]
            if not seen[i]: seen[i] = True; out.append(i)
            nx = [j for j in nbr[i] if not seen[j]]
            if nx: stack.append(nx[0])
            else: stack.pop()
    return [ts[i] for i in out]
class Bits:
    def __init__(self): self.b = []
    def put(self, v, n):
        for i in range(n - 1, -1, -1): self.b.append((v >> i) & 1)
    def eg(self, v, k):
        v += 1 << k; L = v.bit_length(); self.put(0, L - k - 1); self.put(v, L)
    def chars(self):
        while len(self.b) % 13: self.b.append(0)
        out = []
        for i in range(0, len(self.b), 13):
            v = 0
            for x in self.b[i:i + 13]: v = v * 2 + x
            out.append(ALPHA[v // 93] + ALPHA[v % 93])
        return "".join(out)
ALPHA = [chr(c) for c in range(32, 127) if c not in (34, 92)]   # 93 digits
def digit(c): c = ord(c); return c - 32 - (c > 34) - (c > 92)
def zz(v): return 2 * v if v >= 0 else -2 * v - 1
START = ((2048, 1024, 2048), 1)
def encode_fill(ts):
    bs = Bits(); ring = {}; occ = {}; pocc = {}; cur = 0; lastnew = START; news = 0; out_tris = []
    for t in ts:
        r = next((j for j in range(3) if t[j] in occ), 0)
        t = t[r:] + t[:r]; out_tris.append(t)
        pred = None
        for k in t:
            if k in occ:
                d = cur - occ[k]; bs.put(1, 1); bs.eg(d - 1, KD)
                ent = ring.get(occ[k]) if d <= RING else None
            else:
                bs.put(0, 1); news += 1
                pk = (k[0], k[2]); j = pocc.get(pk)
                if j is not None and cur - j <= RING and ring.get(j) == pk:
                    bs.put(1, 1); bs.eg(cur - j - 1, KD)
                else:
                    bs.put(0, 1)
                    pp, pb = pred if pred else lastnew
                    for i in range(3): bs.eg(zz(k[0][i] - pp[i]), KP)
                    if k[2] == pb: bs.put(1, 1)
                    else: bs.put(0, 1); bs.put(k[2], 4)
                bs.put(k[1][0], NB); bs.put(k[1][1], NB)
                ent = pk; lastnew = pk
            ring[cur] = ent
            if ent is not None: pred = ent; pocc[ent] = cur
            occ[k] = cur; cur += 1
    return bs.chars(), out_tris, news
# The decoder, as the game's (digger.coax dg_draw), to check the stream gives back every corner.
def decode_fill(s, ntri):
    vals = [digit(s[i]) * 93 + digit(s[i + 1]) for i in range(0, len(s), 2)]
    bits = [(v >> i) & 1 for v in vals for i in range(12, -1, -1)]; pos = [0]
    def get(n):
        v = 0
        for _ in range(n): v = v * 2 + bits[pos[0]]; pos[0] += 1
        return v
    def eg(k):
        z = 0
        while get(1) == 0: z += 1
        return ((1 << (z + k)) | get(z + k)) - (1 << k)
    def unz(v): return v >> 1 if v % 2 == 0 else -(v + 1) // 2
    out = []; keys = []; ring = {}; lastnew = START; cur = 0
    for _ in range(ntri):
        pred = None; tri_ = []
        for c in range(3):
            if get(1):
                d = eg(KD) + 1; k = keys[cur - d]; ent = ring.get(cur - d) if d <= RING else None
            else:
                if get(1):
                    d = eg(KD) + 1; pk = ring[cur - d]
                else:
                    pp, pb = pred if pred else lastnew
                    q = tuple(pp[i] + unz(eg(KP)) for i in range(3))
                    b = pb if get(1) else get(4)
                    pk = (q, b)
                nrm = (get(NB), get(NB)); k = (pk[0], nrm, pk[1]); ent = pk; lastnew = pk
            ring[cur] = ent
            if ent is not None: pred = ent
            keys.append(k); tri_.append(k); cur += 1
        out.append(tri_)
    return out
streams = []; fill_end = []; body_end = []; tri_total = 0; new_total = 0
for f in range(4):
    ft = [[vkey(i) for i in idx] for c, idx in tris if c == f]
    body = [t for t in ft if all(k[2] < 4 for k in t)]; legs = [t for t in ft if any(k[2] >= 4 for k in t)]
    ordered = walk_order(body) + walk_order(legs)
    s, rot, news = encode_fill(ordered)
    assert decode_fill(s, len(rot)) == rot, "fill %d does not decode" % f
    streams.append(s); body_end.append(tri_total + len(body)); tri_total += len(ordered); fill_end.append(tri_total); new_total += news
counts = [fill_end[0]] + [fill_end[k] - fill_end[k - 1] for k in range(1, 4)]
fill_char = [0]
for s in streams: fill_char.append(fill_char[-1] + len(s))
stream = "".join(streams)
print("stream chars", len(stream), "new vertices", new_total, "fill ends", fill_end, "body ends", body_end)

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
PIECE = 8192   # characters in one piece of the stream, one line of C each (a line the exporter reads is at most 64 KB)
with open(out_coax, "w") as f:
    f.write("""// The Digger's mesh (GitHub #13505): the designer's Tripo model, decimated to %d triangles (normals
// smooth by angle) and baked to one flat base colour in Blender, each triangle given the nearest of four
// cel fills (yellow, ink, cream, steel) so the game's cel pass and ink overlay light it as they light
// the kit. Written by metaverse/splice-bots/tools/digger-rig.py from the rigged model's source
// (metaverse/splice-bots/assets/digger-rigged.glb); do not edit by hand. Model units: 1 = %.3f m long,
// the nose (the drill box) ahead (+z).
// The triangles are one bit stream a fill, read in order as digger.coax draws them (the script says
// how a corner is written): a corner copies the vertex an earlier corner wrote, or is a new vertex.
// DGM_FILL_END[k] is where fill k's triangles end, DGM_BODY_END[k] where its legs' begin, and
// DGM_FILL_CHAR[k] the character fill k's stream starts on; the stream is dgm_s's pieces of DGM_PIECE.
DGM_SPAN :: %.3f;
DGM_NB :: %d; DGM_KD :: %d; DGM_KP :: %d; DGM_RING :: %d;
DGM_VERTS :: %d; DGM_TRIS :: %d; DGM_CHARS :: %d;
DGM_FILL_END :: [4] int.[%s];
DGM_BODY_END :: [4] int.[%s];
DGM_FILL_CHAR :: [5] int.[%s];
DGM_BONES :: %d;
// Each bone: its parent (-1: none), its pivot (where it turns) in model units.
DGM_PARENT :: [DGM_BONES] int.[%s];
DGM_PIVOT :: (b: int) -> Point {
  pivots := [DGM_BONES] Point.[%s];
  return pivots[b];
}
DGM_PIECE :: %d;
""" % (tri_total, LEN, SPAN, NB, KD, KP, RING, new_total, tri_total, len(stream),
       ", ".join(map(str, fill_end)), ", ".join(map(str, body_end)), ", ".join(map(str, fill_char)), len(bones),
       ", ".join(str(b[1]) for b in bones), ", ".join(pt(b[2]) for b in bones), PIECE))
    f.write("dgm_s :: (k: int) -> string {\n")
    for k in range(0, len(stream), PIECE):
        f.write("  if k == %d { return \"%s\"; }\n" % (k // PIECE, stream[k:k + PIECE]))
    f.write("  return \"\";\n}\n")
print("LEN", LEN, "HT", HT, "chars", len(stream))
