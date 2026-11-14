# Firm reason: 6 (Blender's Python API, bpy: the FBX and glTF importers and the mesh tools exist only there)
# Checks a character model before it is rigged or put in a scene (GitHub #13585): a generated or
# hand-made GLB or FBX. It prints one JSON report and exits 1 when a rule fails.
#   Blender -b --factory-startup -P character-check.py -- IN.glb|IN.fbx [--max-tris N] [--out REPORT.json]
# What it measures:
#   size       the file's bytes, triangles (quads count two), quads, vertices, materials, each image's pixels
#   pieces     connected pieces after welding vertices at one place: a clean character is one piece or a
#              few large ones; a small loose piece (under 1 % of the triangles) is named a fragment
#   scale      height in metres, feet at 0, centred; a person is 1.2 to 2.2 m (anything else is the wrong unit)
#   limbs      cuts across the legs and the arms: two legs apart at the shin and the knee, each arm apart
#              from the trunk (a T or an A pose). A cut with too few loops is a fused limb
#   skeleton   bones, which naming it follows, the joints found (character_joints below), which way it
#              faces (foot to toe), the arms' angle below level (0: a T pose), vertices with no weight and
#              with more than four
# character_joints is the one table of bone names: character-fit.py reads it to put a model on our skeleton.
import bpy, bmesh, sys, os, json, math, re
from mathutils import Vector

# The joints our clips need, by the names the Quaternius Universal Animation Library uses (the
# skeleton Everglade's people are on, metaverse/tools/people.py), and the words other rigs use for
# them (Unreal's mannequin, Mixamo, Unity's humanoid, Tripo's, Blender's own).
ALIASES = {
    "pelvis": ["pelvis", "hips", "hip"], "neck_01": ["neck", "neck01", "neck1"], "Head": ["head"],
    "clavicle": ["clavicle", "shoulder", "collar"], "upperarm": ["upperarm", "arm", "uparm"],
    "lowerarm": ["lowerarm", "forearm"], "hand": ["hand", "wrist"],
    "thigh": ["thigh", "upleg", "upperleg"], "calf": ["calf", "shin", "leg", "lowerleg"],
    "foot": ["foot", "ankle"], "ball": ["ball", "toe", "toes", "toebase"],
}
SPINE = ["spine", "chest", "upperchest", "torso", "waist"]
SIDED = ["clavicle", "upperarm", "lowerarm", "hand", "thigh", "calf", "foot", "ball"]
NEEDED = ["pelvis", "Head"] + [b + "_" + s for b in ("upperarm", "lowerarm", "hand", "thigh", "calf", "foot") for s in "lr"]

def bone_word(name):
    """A bone's name without its namespace, side and digits: ("upperarm", "l"), ("spine", "")."""
    n = name.split(":")[-1].split("|")[-1]
    side = ""
    m = re.match(r"^(left|right)[_ .-]?(.+)$", n, re.I) or re.match(r"^(l|r)[_ .-](.+)$", n, re.I)
    if m: side, n = m.group(1)[0].lower(), m.group(2)
    else:
        m = re.match(r"^(.+?)[_ .-]?(left|right)$", n, re.I) or re.match(r"^(.+)[_ .-](l|r)$", n, re.I)
        if m: n, side = m.group(1), m.group(2)[0].lower()
    return re.sub(r"[^a-z]", "", n.lower()), side

def character_joints(arm):
    """Our joint name -> the armature's bone name. A side is where the bone is on the figure, never
    its name: the figure's left is to the left of the way its feet point (foot to toe), so a model
    turned any way about is read the same. With no toes, the names' sides are used. The spine is
    the chain of spine bones from the pelvis up, in order, under "spine"."""
    out = {}; spine = []; found = {}
    head = lambda n: arm.matrix_world @ arm.data.bones[n].head_local
    for b in arm.data.bones:   # parents come first
        w, side = bone_word(b.name)
        if w in SPINE: spine.append(b.name); continue
        for joint, words in ALIASES.items():
            if w in words: found.setdefault(joint, []).append((b.name, side))
    for joint in ("pelvis", "neck_01", "Head"):
        if joint in found: out[joint] = found[joint][0][0]
    left = None
    if found.get("foot") and found.get("ball"):
        f = sum((head(n) for n, _ in found["ball"]), Vector()) / len(found["ball"]) - sum((head(n) for n, _ in found["foot"]), Vector()) / len(found["foot"])
        f.z = 0
        if f.length > 1e-4: left = Vector((0, 0, 1)).cross(f.normalized())
    mid = head(out["pelvis"]) if "pelvis" in out else Vector()
    for joint in SIDED:
        for n, side in found.get(joint, []):
            if left is not None: side = "l" if (head(n) - mid).dot(left) > 0 else "r"
            elif not side: side = "l" if head(n).x > 0 else "r"
            out.setdefault(joint + "_" + side, n)
    if spine: out["spine"] = spine
    return out

def naming(arm):
    names = [b.name for b in arm.data.bones]
    if any(n.lower().startswith("mixamorig") for n in names): return "mixamo"
    if "spine_01" in names and "upperarm_l" in names: return "unreal"
    if "Hips" in names and "LeftUpperArm" in names: return "unity"
    return "other"

def load(path):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    ext = os.path.splitext(path)[1].lower()
    if ext in (".glb", ".gltf"): bpy.ops.import_scene.gltf(filepath=path)
    elif ext == ".fbx": bpy.ops.import_scene.fbx(filepath=path)
    else: raise SystemExit("character-check: a .glb, .gltf or .fbx file, not " + ext)
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    arms = [o for o in bpy.context.scene.objects if o.type == "ARMATURE"]
    arm = arms[0] if arms else None
    skinned = [o for o in meshes if arm and o.find_armature() == arm]
    return (skinned or meshes), arm, len(meshes) - len(skinned or meshes)

def welded(meshes):
    """Every mesh in world space as one bmesh, vertices at one place welded."""
    bm = bmesh.new()
    for o in meshes:
        t = bmesh.new(); t.from_mesh(o.data); t.transform(o.matrix_world)
        me = bpy.data.meshes.new("t"); t.to_mesh(me); t.free(); bm.from_mesh(me); bpy.data.meshes.remove(me)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    return bm

def pieces(bm):
    """The triangles of each connected piece, largest first."""
    bm.verts.ensure_lookup_table()
    seen = set(); out = []
    for v in bm.verts:
        if v.index in seen: continue
        stack = [v]; seen.add(v.index); faces = set()
        while stack:
            a = stack.pop()
            for f in a.link_faces: faces.add(f.index)
            for e in a.link_edges:
                b = e.other_vert(a)
                if b.index not in seen: seen.add(b.index); stack.append(b)
        bm.faces.ensure_lookup_table()
        out.append(sum(len(bm.faces[i].verts) - 2 for i in faces))
    return sorted(out, reverse=True)

def cut_loops(bm, co, no):
    """How many separate outlines a plane cuts from the surface."""
    c = bm.copy()
    r = bmesh.ops.bisect_plane(c, geom=c.verts[:] + c.edges[:] + c.faces[:], plane_co=co, plane_no=no, dist=1e-6)
    edges = [g for g in r["geom_cut"] if isinstance(g, bmesh.types.BMEdge)]
    par = {}
    def find(a):
        while par.setdefault(a, a) != a: par[a] = par[par[a]]; a = par[a]
        return a
    for e in edges: par[find(e.verts[0].index)] = find(e.verts[1].index)
    n = len({find(a) for a in list(par)})
    c.free()
    return n

def report(path, max_tris=0):
    meshes, arm, extra = load(path)
    fails = []; r = {"file": os.path.basename(path), "bytes": os.path.getsize(path), "meshes_not_skinned": extra}
    if not meshes: return {"file": r["file"], "fails": ["no mesh"]}
    tris = quads = ngons = verts = 0
    for o in meshes:
        verts += len(o.data.vertices)
        for p in o.data.polygons:
            n = len(p.vertices); tris += n - 2; quads += n == 4; ngons += n > 4
    r.update(meshes=len(meshes), vertices=verts, triangles=tris, quads=quads, ngons=ngons)
    if max_tris and tris > max_tris: fails.append("%d triangles, over the budget of %d" % (tris, max_tris))
    mats = {m.name for o in meshes for m in o.data.materials if m}
    r["materials"] = len(mats)
    r["images"] = [{"name": i.name, "w": i.size[0], "h": i.size[1]} for i in bpy.data.images if i.size[0] > 0]

    bm = welded(meshes)
    ps = pieces(bm)
    small = [p for p in ps if p < 0.01 * tris]
    r["pieces"] = {"count": len(ps), "largest": ps[:6], "fragments": len(small), "fragment_triangles": sum(small)}
    if small: fails.append("%d loose fragments (%d triangles)" % (len(small), sum(small)))
    r["open_edges"] = sum(1 for e in bm.edges if len(e.link_faces) == 1)
    r["bad_edges"] = sum(1 for e in bm.edges if len(e.link_faces) > 2)

    lo = Vector((min(v.co.x for v in bm.verts), min(v.co.y for v in bm.verts), min(v.co.z for v in bm.verts)))
    hi = Vector((max(v.co.x for v in bm.verts), max(v.co.y for v in bm.verts), max(v.co.z for v in bm.verts)))
    h = hi.z - lo.z
    r["bounds"] = {"min": [round(x, 4) for x in lo], "max": [round(x, 4) for x in hi], "height": round(h, 4)}
    if not 1.2 <= h <= 2.2: fails.append("height %.3f m: not a person's (the wrong unit or the wrong up axis)" % h)
    if abs(lo.z) > 0.03 * h: fails.append("feet at %.3f m, not on the ground" % lo.z)
    span = hi.x - lo.x
    r["pose"] = "T" if span > 0.8 * h else ("A" if span > 0.45 * h else "arms down")

    cx = (lo.x + hi.x) / 2
    legs = {k: cut_loops(bm, Vector((0, 0, lo.z + f * h)), Vector((0, 0, 1))) for k, f in (("ankle", 0.06), ("shin", 0.15), ("knee", 0.28), ("thigh", 0.38))}
    arms = {k: cut_loops(bm, Vector((cx + s * 0.32 * span, 0, 0)), Vector((1, 0, 0))) for k, s in (("right", -1), ("left", 1))} if span > 0.45 * h else {}
    r["limbs"] = {"leg_loops": legs, "arm_loops": arms}
    for k in ("shin", "knee"):
        if legs[k] < 2: fails.append("the legs are one piece at the %s (%d outline)" % (k, legs[k]))
    for k, n in arms.items():
        if n < 1: fails.append("no %s arm out from the trunk" % k)
    if not arms: fails.append("arms down against the body: a T or an A pose rigs; this will fuse the arms to the trunk")
    bm.free()

    if arm:
        j = character_joints(arm); W = arm.matrix_world
        head = lambda n: W @ arm.data.bones[n].head_local
        sk = {"bones": len(arm.data.bones), "naming": naming(arm), "joints": {k: v for k, v in j.items()},
              "missing": [n for n in NEEDED if n not in j] + ([] if j.get("spine") else ["spine"])}
        if sk["missing"]: fails.append("joints not found: " + ", ".join(sk["missing"]))
        if "foot_l" in j and "ball_l" in j:
            d = head(j["ball_l"]) - head(j["foot_l"]); d.z = 0
            if d.length > 1e-4:
                d.normalize(); sk["faces"] = [round(d.x, 3), round(d.y, 3)]
                sk["turn_to_front_deg"] = round(math.degrees(-math.atan2(d.x, -d.y)), 1)   # the turn about z that faces it to -y, Blender's front; 0: it does
        if "upperarm_l" in j and "lowerarm_l" in j:
            d = head(j["lowerarm_l"]) - head(j["upperarm_l"])
            sk["arm_below_level_deg"] = round(math.degrees(math.atan2(-d.z, abs(d.x))), 1)
        none = many = 0
        for o in meshes:
            names = {g.index: g.name for g in o.vertex_groups}
            for v in o.data.vertices:
                n = sum(1 for g in v.groups if g.weight > 1e-4 and names.get(g.group) in arm.data.bones)
                none += n == 0; many += n > 4
        sk["unweighted_vertices"] = none; sk["over_four_weights"] = many
        if none: fails.append("%d vertices follow no bone" % none)
        r["skeleton"] = sk
        r["clips"] = [a.name for a in bpy.data.actions]
    else:
        r["skeleton"] = None
    r["fails"] = fails
    return r

if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if not argv: raise SystemExit("usage: Blender -b --factory-startup -P character-check.py -- IN.glb|IN.fbx [--max-tris N] [--out REPORT.json]")
    opt = lambda k, d: argv[argv.index(k) + 1] if k in argv else d
    r = report(argv[0], int(opt("--max-tris", 0)))
    text = json.dumps(r, indent=1)
    if opt("--out", ""): open(opt("--out", ""), "w").write(text + "\n")
    print("CHARACTER_CHECK " + text)
    sys.exit(1 if r["fails"] else 0)
