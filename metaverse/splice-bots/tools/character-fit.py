# Firm reason: 6 (Blender's Python API, bpy: the FBX and glTF importers, the armature and the glTF export exist only there)
# Puts a rigged character on our skeleton and bakes our clips onto it (GitHub #13585). The model is
# a GLB or FBX with any biped skeleton whose joints character-check.py finds (Unreal's mannequin,
# Mixamo, Unity's humanoid, Tripo's, a hand-made one). Our skeleton is the Quaternius Universal
# Animation Library's (65 joints; the one Everglade's people are on, metaverse/tools/people.py), so the
# library's clips and every clip made for Everglade play on the result.
#   Blender -b --factory-startup -P character-fit.py -- IN.glb|IN.fbx LIBRARY.glb OUT.glb OUT.json
#           [--height M] [--clips A,B,...] [--reach M] [--fbx DIR]
# --fills PALETTE.json (with --fbx) gives the FBX mesh flat fills in place of its texture, for the
# game's own cel material in Unreal (M_AeCel reads a vertex's colour, its material from the second
# UV set's u and its parameter from the first's): the texture is read once at each triangle, the
# triangles are grouped into a few colours (k-means, then each takes the majority of its
# neighbours, so a region has no speckle), and each group takes the palette entry nearest it:
#   [{"near": [r, g, b], "to": [r, g, b], "mat": 10, "back": true}, ...]   colours 0 to 1, sRGB
# "to" is the fill, "mat" the cel material (metaverse-engine/render/cel-mesh.coax: 1 skin, 2 hair,
# 10 cloth). An entry with "back" also takes skin-coloured triangles behind her head (a mane's
# underside holds whatever the texture had there).
# --fbx DIR also writes FBX files for Unreal: DIR/NAME.fbx (the mesh on the skeleton, no clip) and
# DIR/NAME@CLIP.fbx (the skeleton and one clip each), NAME being OUT's. The bridge's "thing." row
# takes the first as a skeletal mesh and the others as its "anim." rows (metaverse-engine/export/unreal/README.md).
# LIBRARY.glb is the animation library (UAL1_Standard.glb); it is not in the repository.
# What it does, in order:
#   1. stands the model up: turned to face -y (foot to toe), scaled to --height (default: its own
#      when a person's, else 1.75 m), feet on the ground, the pelvis over the origin;
#   2. finds each of our joints on its skeleton (character_joints). A missing clavicle, neck or
#      toe is placed from its neighbours; a missing arm, leg, pelvis or head stops the fit;
#   3. builds our armature with every bone at her joint. A limb that is not where the library's is
#      (arms down or in an A where the library stands in a T) gets a correction K, so a clip's pose
#      is the same pose on her (target local = target rest x source rest^-1 x source local);
#   4. moves each vertex's weights to our bones: a bone of hers that is one of ours by name, else
#      its nearest parent that is (fingers to the hand, a twist bone to its limb). The spine's
#      bones go to spine_01, spine_02 and spine_03 in order; fewer than three are split by height.
#      --reach M drops an influence from a bone farther than M from the vertex (a stray weight);
#   5. bakes the clips at 30 a second: rotations, and the pelvis's movement scaled by her legs;
#   6. writes the GLB and a JSON of what it did (joints, clips, how many weights it moved).
# It changes no weight beyond that. A model whose arms hang against a fused jacket needs its
# weights repaired by hand or the mesh made again in a T pose; the report says how many vertices
# sit under two limbs so that is seen before anyone looks at a clip.
import bpy, sys, os, json, math
from mathutils import Vector, Quaternion, Matrix
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import importlib
cc = importlib.import_module("character-check")

argv = sys.argv[sys.argv.index("--") + 1:]
SRC, LIB, OUT, OUTJ = argv[:4]
opt = lambda k, d: argv[argv.index(k) + 1] if k in argv else d
CLIPS = opt("--clips", "Idle_Loop,Walk_Loop,Jog_Fwd_Loop,Sprint_Loop,Jump_Start,Jump_Loop,Jump_Land,Crouch_Idle_Loop,Crouch_Fwd_Loop").split(",")
HEIGHT = float(opt("--height", 0)); REACH = float(opt("--reach", 0)); FPS_OUT = 30
FINGERS = ("index", "middle", "pinky", "ring", "thumb")

# ---- 1. the model, standing as the library stands
meshes, H, _ = cc.load(SRC)
if not H: raise SystemExit("character-fit: no skeleton in " + SRC + " (rig it first)")
for o in list(bpy.data.objects):
    if o.type == "MESH" and o not in meshes: bpy.data.objects.remove(o, do_unlink=True)
bpy.context.view_layer.update()
if len(meshes) > 1:
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes: o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]; bpy.ops.object.join()
char = meshes[0]
J = cc.character_joints(H)
missing = [n for n in cc.NEEDED if n not in J] + ([] if J.get("spine") else ["spine"])
if missing: raise SystemExit("character-fit: joints not found: " + ", ".join(missing) + "; bones: " + ", ".join(b.name for b in H.data.bones))
hw = lambda n: H.matrix_world @ H.data.bones[n].head_local
turn = 0.0
toe = (hw(J["ball_l"]) - hw(J["foot_l"])) if "ball_l" in J else None
if toe is not None and Vector((toe.x, toe.y)).length > 1e-4: turn = -math.atan2(toe.x, -toe.y)   # the turn about z that points her toes at -y
P = [char.matrix_world @ v.co for v in char.data.vertices]
z0 = min(p.z for p in P); h0 = max(p.z for p in P) - z0
scale = (HEIGHT / h0) if HEIGHT else (1.0 if 1.2 <= h0 <= 2.2 else 1.75 / h0)
pel = hw(J["pelvis"])
N = Matrix.Scale(scale, 4) @ Matrix.Rotation(turn, 4, "Z") @ Matrix.Translation(Vector((-pel.x, -pel.y, -z0)))
roots = [o for o in (char, H) if o.parent is None or o.parent not in (char, H)]
tops = set()
for o in roots:
    while o.parent: o = o.parent
    tops.add(o)
for o in tops: o.matrix_world = N @ o.matrix_world
bpy.context.view_layer.update()
hh = lambda n: H.matrix_world @ H.data.bones[n].head_local
print("STOOD turn", round(math.degrees(turn), 1), "scale", round(scale, 4), "height", round(h0 * scale, 3))

# ---- the library's skeleton
before = set(bpy.data.objects)
bpy.ops.import_scene.gltf(filepath=LIB)
new = [o for o in bpy.data.objects if o not in before]
M = [o for o in new if o.type == "ARMATURE"][0]
for o in new:
    if o.type == "MESH": bpy.data.objects.remove(o, do_unlink=True)
src_fps = bpy.context.scene.render.fps / bpy.context.scene.render.fps_base
MW = M.matrix_world
mpos = {b.name: MW @ b.head_local for b in M.data.bones}
mframe = {b.name: (MW @ b.matrix_local).to_quaternion() for b in M.data.bones}
order = [b.name for b in M.data.bones]   # parents come first

# ---- 2. her joints, by our names
sp = J["spine"]
T = {"root": Vector((0, 0, 0)), "pelvis": hh(J["pelvis"]), "Head": hh(J["Head"]), "spine_01": hh(sp[0])}
T["spine_03"] = hh(sp[-1]) if len(sp) > 1 else T["spine_01"].lerp(T["Head"], 0.45)
T["spine_02"] = hh(sp[len(sp) // 2]) if len(sp) > 2 else (T["spine_01"] + T["spine_03"]) / 2
T["neck_01"] = hh(J["neck_01"]) if "neck_01" in J else T["spine_03"].lerp(T["Head"], 0.75)
placed = [n for n in ("neck_01",) if n not in J]
for s in "lr":
    for b in ("upperarm", "lowerarm", "hand", "thigh", "calf", "foot"): T[b + "_" + s] = hh(J[b + "_" + s])
    if "clavicle_" + s in J: T["clavicle_" + s] = hh(J["clavicle_" + s])
    else: T["clavicle_" + s] = T["spine_03"].lerp(T["upperarm_" + s], 0.25); placed.append("clavicle_" + s)
    if "ball_" + s in J: T["ball_" + s] = hh(J["ball_" + s])
    else:
        f = T["foot_" + s]; T["ball_" + s] = Vector((f.x, f.y - 0.6 * f.z - 0.06, 0.02)); placed.append("ball_" + s)
# The directions that set each bone's swing: (the library's, hers).
D = {}
def pair(n, mc, hv): D[n] = (mpos[mc] - mpos[n], hv)
for a, b in (("pelvis", "spine_01"), ("spine_01", "spine_02"), ("spine_02", "spine_03"), ("spine_03", "neck_01"), ("neck_01", "Head")):
    pair(a, b, T[b] - T[a])
head_len = (MW @ M.data.bones["Head"].tail_local) - mpos["Head"]
D["Head"] = (head_len, Vector((0, 0, max(h0 * scale - T["Head"].z, 0.08))))
for s in "lr":
    for a, b in (("clavicle_", "upperarm_"), ("upperarm_", "lowerarm_"), ("lowerarm_", "hand_"), ("thigh_", "calf_"), ("calf_", "foot_"), ("foot_", "ball_")):
        pair(a + s, b + s, T[b + s] - T[a + s])
    fore = (T["hand_" + s] - T["lowerarm_" + s]).normalized()
    D["hand_" + s] = (mpos["middle_01_" + s] - mpos["hand_" + s], fore * 0.4 * (T["hand_" + s] - T["lowerarm_" + s]).length)
    D["ball_" + s] = (mpos["ball_leaf_" + s] - mpos["ball_" + s], Vector((0, -(mpos["ball_leaf_" + s] - mpos["ball_" + s]).length, 0)))

# ---- 3. our armature at her joints
LIMB = {"upperarm", "lowerarm", "hand", "thigh", "calf"}
K = {"root": Quaternion()}
for n in order:
    base = n.rsplit("_", 1)[0]
    if base in LIMB and n in D: K[n] = D[n][0].normalized().rotation_difference(D[n][1].normalized())
    elif any(k in n for k in FINGERS): K[n] = K[M.data.bones[n].parent.name]
    else: K[n] = Quaternion()
def group_scale(n):
    s = n[-1]
    if any(k in n for k in FINGERS): return D["hand_" + s][1].length / D["hand_" + s][0].length
    if n.startswith("ball_leaf"): return 1.0
    return 1.0
for n in order:
    if n in T: continue
    p = M.data.bones[n].parent.name
    T[n] = T[p] + (K[p] @ (mpos[n] - mpos[p])) * group_scale(n)
leg_m = (mpos["thigh_l"] - mpos["calf_l"]).length + (mpos["calf_l"] - mpos["foot_l"]).length
leg_h = (T["thigh_l"] - T["calf_l"]).length + (T["calf_l"] - T["foot_l"]).length
S_LEG = leg_h / leg_m
bpy.ops.object.select_all(action="DESELECT")
Q = M.copy(); Q.data = M.data.copy(); Q.name = "Rig"; Q.data.name = "Rig"
Q.animation_data_clear(); bpy.context.scene.collection.objects.link(Q)
Q.matrix_world = Matrix.Identity(4)
bpy.context.view_layer.objects.active = Q; Q.select_set(True)
bpy.ops.object.mode_set(mode="EDIT")
for eb in Q.data.edit_bones: eb.use_connect = False
for n in order:
    eb = Q.data.edit_bones[n]
    L = M.data.bones[n].length
    if n in D and D[n][0].length > 1e-6: L *= D[n][1].length / D[n][0].length
    else: L *= group_scale(n)
    eb.matrix = Matrix.Translation(T[n]) @ (K[n] @ mframe[n]).to_matrix().to_4x4()
    eb.length = max(L, 0.01)
bpy.ops.object.mode_set(mode="OBJECT")

# ---- 4. the weights, onto our bones
ours = set(order)
to = {}   # her bone -> ours
for k, v in J.items():
    if k != "spine": to[v] = k
if len(sp) >= 3:
    for i, n in enumerate(sp): to[n] = ("spine_01", "spine_02", "spine_03")[round(i * 2 / (len(sp) - 1))]
elif len(sp) == 2: to[sp[1]] = "spine_03"
split = sp[0] if len(sp) < 3 else None       # this one is shared out by height
split_to = ("spine_01", "spine_02") if len(sp) == 2 else ("spine_01", "spine_02", "spine_03")
for b in H.data.bones:   # parents first, so a parent's answer is there
    if b.name in to or b.name == split: continue
    w, _ = cc.bone_word(b.name)
    side = "_l" if hh(b.name).x > 0 else "_r"
    guess = None
    for f in FINGERS:   # index_01_l and Mixamo's LeftHandIndex1 alike
        if f in w:
            digits = "".join(c for c in b.name if c.isdigit())[-1:] or "1"
            guess = "%s_%02d%s" % (f, int(digits), side)
    if b.name in ours: guess = b.name
    if guess in ours: to[b.name] = guess
    elif b.parent: to[b.name] = to.get(b.parent.name) or ("__split" if b.parent.name == split else "pelvis")
    else: to[b.name] = "root"
mw = char.matrix_world
names = {g.index: g.name for g in char.vertex_groups}
PV = [mw @ v.co for v in char.data.vertices]
zs = [T[n].z for n in split_to]
def by_height(z):
    if z <= zs[0]: return {split_to[0]: 1.0}
    for i in range(len(zs) - 1):
        if z <= zs[i + 1]: t = (z - zs[i]) / max(zs[i + 1] - zs[i], 1e-6); return {split_to[i]: 1 - t, split_to[i + 1]: t}
    return {split_to[-1]: 1.0}
W = []
for v in char.data.vertices:
    w = {}
    for g in v.groups:
        n = names[g.group]
        if g.weight <= 0 or n not in H.data.bones: continue
        t = to.get(n, "pelvis")
        if n == split or t == "__split":
            for k, f in by_height(PV[v.index].z).items(): w[k] = w.get(k, 0) + g.weight * f
        else: w[t] = w.get(t, 0) + g.weight
    W.append(w)
SEG = {b.name: (b.head_local.copy(), b.tail_local.copy()) for b in Q.data.bones if b.name != "root"}
def seg_d(p, n):
    A, B = SEG[n]; ab = B - A
    t = max(0.0, min(1.0, (p - A).dot(ab) / max(ab.length_squared, 1e-9)))
    return (p - (A + ab * t)).length
stray = unbound = 0
for i, w in enumerate(W):
    if REACH > 0:
        bad = [n for n in w if n in SEG and seg_d(PV[i], n) > REACH]
        if bad:
            stray += 1; lost = sum(w.pop(n) for n in bad); s0 = sum(w.values())
            if s0 > 0:
                for n in w: w[n] += lost * w[n] / s0
            else: w[min(SEG, key=lambda n: seg_d(PV[i], n))] = lost
    if not w: unbound += 1; w[min(SEG, key=lambda n: seg_d(PV[i], n))] = 1.0
# A vertex an arm and the trunk (or both legs) pull at once: where a clip will tear a fused surface.
ARM = lambda n: n.startswith(("upperarm", "lowerarm"))
TRUNK = ("pelvis", "spine_01", "spine_02", "spine_03")
shared_arm = sum(1 for w in W if any(ARM(n) and x > 0.15 for n, x in w.items()) and sum(x for n, x in w.items() if n in TRUNK) > 0.15 * sum(w.values()))
shared_leg = sum(1 for w in W if min(sum(x for n, x in w.items() if n in (a + "_l" for a in ("thigh", "calf"))), sum(x for n, x in w.items() if n in (a + "_r" for a in ("thigh", "calf")))) > 0.15 * sum(w.values()))
for vg in list(char.vertex_groups): char.vertex_groups.remove(vg)
gi = {}
for i, w in enumerate(W):
    top = sorted(w.items(), key=lambda kv: -kv[1])[:4]; s = sum(x for _, x in top) or 1.0
    for n, x in top:
        if x / s < 1e-3: continue
        if n not in gi: gi[n] = char.vertex_groups.new(name=n)
        gi[n].add([i], x / s, "REPLACE")
wm = char.matrix_world.copy()
char.parent = Q; char.matrix_world = wm
for m in list(char.modifiers):
    if m.type == "ARMATURE": char.modifiers.remove(m)
char.modifiers.new("Armature", "ARMATURE").object = Q
bpy.data.objects.remove(H, do_unlink=True)
print("WEIGHTS bones", len(gi), "stray", stray, "unbound", unbound, "arm+trunk", shared_arm, "both legs", shared_leg)

# ---- 5. the clips
sc = bpy.context.scene
sc.render.fps = FPS_OUT; sc.render.fps_base = 1
qrest = {b.name: b.matrix_local.to_quaternion() for b in Q.data.bones}
qpar = {b.name: (b.parent.name if b.parent else None) for b in Q.data.bones}
Q.animation_data_create()
info = {"source": os.path.basename(SRC), "turned_deg": round(math.degrees(turn), 2), "scale": scale, "height": h0 * scale,
        "leg_scale": S_LEG, "joints_placed": placed, "her_bones": to, "weights": {"stray": stray, "unbound": unbound,
        "arm_and_trunk": shared_arm, "both_legs": shared_leg, "vertices": len(W)}, "clips": {}, "joints": {n: list(T[n]) for n in order}}
for name in CLIPS:
    act = bpy.data.actions.get(name)
    if not act: print("MISSING", name); continue
    M.animation_data.action = act
    if hasattr(act, "slots") and len(act.slots): M.animation_data.action_slot = act.slots[0]
    f0, f1 = act.frame_range
    dur = (f1 - f0) / src_fps; n_out = max(1, round(dur * FPS_OUT))
    out = bpy.data.actions.new("Q_" + name)
    Q.animation_data.action = out
    prev = {}
    for k in range(n_out + 1):
        f = f0 + (f1 - f0) * k / n_out
        sc.frame_set(int(math.floor(f)), subframe=f - math.floor(f))
        Pm = {pb.name: (MW @ pb.matrix).to_quaternion() for pb in M.pose.bones}
        for pb in Q.pose.bones:
            n = pb.name; p = qpar[n]
            q = (qrest[n].inverted() @ Pm[n]) if p is None else (qrest[n].inverted() @ qrest[p] @ Pm[p].inverted() @ Pm[n])
            q.normalize()
            if n in prev and prev[n].dot(q) < 0: q.negate()
            prev[n] = q
            pb.rotation_mode = "QUATERNION"; pb.rotation_quaternion = q
            pb.keyframe_insert("rotation_quaternion", frame=k)
            if n == "pelvis":
                pb.location = qrest["pelvis"].inverted() @ (((MW @ M.pose.bones["pelvis"].head) - mpos["pelvis"]) * S_LEG)
                pb.keyframe_insert("location", frame=k)
    info["clips"][name] = {"frames": n_out, "seconds": dur}
    tr = Q.animation_data.nla_tracks.new(); tr.name = name
    st = tr.strips.new(name, 0, out); st.action_frame_end = n_out
    Q.animation_data.action = None
    for pb in Q.pose.bones: pb.rotation_quaternion = Quaternion(); pb.location = Vector()
    print("BAKED", name, n_out, round(dur, 3))

# A stance the library has no clip for: on a hoverboard (feet apart along the deck, knees soft, arms out and
# low), set from the rest pose and held, as the clip Board_Stance.
STANCE = [("thigh_l", "X", -30), ("calf_l", "X", 40), ("thigh_r", "X", 10), ("calf_r", "X", 30), ("upperarm_l", "Z", -35), ("upperarm_r", "Z", 35), ("lowerarm_l", "X", 25), ("lowerarm_r", "X", 25)]
out = bpy.data.actions.new("Q_Board_Stance"); Q.animation_data.action = out
for pb in Q.pose.bones:
    pb.rotation_mode = "QUATERNION"; pb.rotation_quaternion = Quaternion(); pb.location = Vector()
for n, ax, deg in STANCE: Q.pose.bones[n].rotation_quaternion = Quaternion({"X": Vector((1, 0, 0)), "Y": Vector((0, 1, 0)), "Z": Vector((0, 0, 1))}[ax], math.radians(deg))
for k in (0, 30):
    for pb in Q.pose.bones: pb.keyframe_insert("rotation_quaternion", frame=k)
    Q.pose.bones["pelvis"].keyframe_insert("location", frame=k)
tr = Q.animation_data.nla_tracks.new(); tr.name = "Board_Stance"
st = tr.strips.new("Board_Stance", 0, out); st.action_frame_end = 30
Q.animation_data.action = None
for pb in Q.pose.bones: pb.rotation_quaternion = Quaternion(); pb.location = Vector()
info["clips"]["Board_Stance"] = {"frames": 30, "seconds": 1.0}

# ---- 6. out
bpy.data.objects.remove(M, do_unlink=True)
bpy.ops.object.select_all(action="DESELECT")
char.select_set(True); Q.select_set(True)
sc.frame_set(0)
bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLB", use_selection=True, export_animations=True,
    export_animation_mode="NLA_TRACKS", export_force_sampling=True, export_frame_step=1,
    export_skins=True, export_image_format="AUTO", export_normals=True, export_yup=True,
    export_def_bones=False, export_anim_single_armature=True)
info["bytes"] = os.path.getsize(OUT)
FBX = opt("--fbx", "")
FILLS = opt("--fills", "")
def flat_fills(ob, palette):
    import numpy as np
    from collections import Counter, defaultdict
    me = ob.data; me.calc_loop_triangles()
    img = None
    for m in me.materials:
        if m and m.node_tree:
            for n in m.node_tree.nodes:
                if n.type == "TEX_IMAGE" and n.image and n.image.size[0] > 1: img = n.image
    if not img or not me.uv_layers.active: raise SystemExit("character-fit: --fills needs the mesh's texture")
    Wd, Hd = img.size
    px = np.empty(Wd * Hd * 4, dtype=np.float32); img.pixels.foreach_get(px); px = px.reshape(Hd, Wd, 4)[:, :, :3]
    uv = me.uv_layers.active.data
    polys = list(me.polygons)
    col = np.zeros((len(polys), 3), dtype=np.float32)
    for pi, pl in enumerate(polys):
        us = np.array([tuple(uv[l].uv) for l in pl.loop_indices]); c = us.mean(0); smp = []
        for u, v in [c] + [c * 0.4 + q * 0.6 for q in us[:4]]:
            smp.append(px[min(Hd - 1, max(0, int((v % 1) * Hd))), min(Wd - 1, max(0, int((u % 1) * Wd)))])
        col[pi] = np.median(np.array(smp), axis=0)
    K = 10; cs = [col[0]]
    while len(cs) < K:
        d = np.min(np.stack([((col - c) ** 2).sum(1) for c in cs]), axis=0); cs.append(col[int(d.argmax())])
    cs = np.array(cs)
    for _ in range(20):
        k = np.argmin(((col[:, None, :] - cs[None, :, :]) ** 2).sum(2), axis=1)
        for i in range(K):
            if (k == i).any(): cs[i] = col[k == i].mean(0)
    P = [ob.matrix_world @ v.co for v in me.vertices]
    key = lambda vi: (round(P[vi].x, 4), round(P[vi].y, 4), round(P[vi].z, 4))
    edge = defaultdict(list)
    for pi, pl in enumerate(polys):
        ks = [key(v) for v in pl.vertices]
        for a in range(len(ks)): edge[tuple(sorted((ks[a], ks[(a + 1) % len(ks)])))].append(pi)
    nb = defaultdict(set)
    for ps in edge.values():
        for a in ps:
            for b in ps:
                if a != b: nb[a].add(b)
    k = list(k)
    for _ in range(4):
        new = []
        for pi in range(len(polys)):
            votes = Counter(k[o] for o in nb[pi]); votes[k[pi]] += 1.5
            new.append(votes.most_common(1)[0][0])
        k = new
    k = np.array(k)
    big = [i for i in range(K) if (k == i).sum() >= 0.005 * len(polys)]
    for i in range(K):
        if i not in big and (k == i).any(): k[k == i] = big[int(((cs[big] - cs[i]) ** 2).sum(1).argmin())]
    for i in big: cs[i] = col[k == i].mean(0)
    entry = {i: min(range(len(palette)), key=lambda e: float(((cs[i] - np.array(palette[e]["near"])) ** 2).sum())) for i in big}
    pe = [entry[int(x)] for x in k]
    back = [e for e in range(len(palette)) if palette[e].get("back")]
    if back:
        yb = Q.data.bones["Head"].head_local.y + 0.03
        for pi, pl in enumerate(polys):
            if palette[pe[pi]].get("mat") == 1 and min(P[v].y for v in pl.vertices) > yb and abs(sum(P[v].x for v in pl.vertices) / len(pl.vertices)) < 0.3: pe[pi] = back[0]
    ca = me.color_attributes.new("Col", "BYTE_COLOR", "CORNER"); me.color_attributes.active_color = ca; me.color_attributes.render_color_index = me.color_attributes.find("Col")
    while len(me.uv_layers) < 2: me.uv_layers.new(name="AeMat")
    u0, u1 = me.uv_layers[0].data, me.uv_layers[1].data
    lin = lambda c: c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    for pi, pl in enumerate(polys):
        e = palette[pe[pi]]; r, g, b = [lin(x) for x in e["to"]]   # Blender keeps a colour attribute linear and writes it sRGB
        for l in pl.loop_indices:
            ca.data[l].color = (r, g, b, 1.0); u0[l].uv = (1.0, 0.0); u1[l].uv = (float(e.get("mat", 10)), 0.0)
    tally = Counter(pe)
    print("FILLS", [(palette[e].get("name", e), tally[e]) for e in sorted(tally)])
    return {str(palette[e].get("name", e)): tally[e] for e in tally}
if FBX:
    os.makedirs(FBX, exist_ok=True)
    if FILLS: info["fills"] = flat_fills(char, json.load(open(FILLS)))
    stem = os.path.splitext(os.path.basename(OUT))[0]
    common = dict(use_selection=True, add_leaf_bones=False, apply_unit_scale=True, apply_scale_options="FBX_SCALE_UNITS",
                  axis_forward="-Z", axis_up="Y", use_armature_deform_only=False, mesh_smooth_type="FACE", path_mode="COPY", embed_textures=True, colors_type="LINEAR")
    tracks = list(Q.animation_data.nla_tracks)
    for tr in tracks: tr.mute = True
    Q.data.pose_position = "REST"; bpy.context.view_layer.update()   # the mesh is bound as it rests, whatever clip was last looked at
    bpy.ops.export_scene.fbx(filepath=os.path.join(FBX, stem + ".fbx"), object_types={"ARMATURE", "MESH"}, bake_anim=False, **common)
    Q.data.pose_position = "POSE"; bpy.context.view_layer.update()
    bpy.ops.object.select_all(action="DESELECT"); Q.select_set(True); bpy.context.view_layer.objects.active = Q
    for tr in tracks:
        act = tr.strips[0].action
        Q.animation_data.action = act
        if hasattr(act, "slots") and len(act.slots): Q.animation_data.action_slot = act.slots[0]
        sc.frame_start = 0; sc.frame_end = int(tr.strips[0].action_frame_end)
        bpy.ops.export_scene.fbx(filepath=os.path.join(FBX, "%s@%s.fbx" % (stem, tr.name)), object_types={"ARMATURE"}, bake_anim=True,
                                 bake_anim_use_all_bones=True, bake_anim_use_nla_strips=False, bake_anim_use_all_actions=False, bake_anim_force_startend_keying=True, bake_anim_simplify_factor=0.0, **common)
    Q.animation_data.action = None
    info["fbx"] = sorted(os.listdir(FBX))
    print("FBX", FBX, len(info["fbx"]), "files")
json.dump(info, open(OUTJ, "w"), indent=1)
print("EXPORTED", OUT, info["bytes"])
