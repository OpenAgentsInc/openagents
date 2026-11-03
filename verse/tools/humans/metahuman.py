"""pack_human.py's MetaHuman source: a MetaHuman export's DNA meshes on the people rig.

    python3 verse/tools/humans/pack_human.py metahuman EXPORT_DIR [OUT.pack]

EXPORT_DIR is a MetaHuman export (~/work/film-out/draconia/metahuman/export): its
dna/*_Body.dna and dna/*_Head.dna and its texture maps. Each DNA is dumped by OpenRigLogic's
dna_dump (MIT; built under ../src/openriglogic/build, or DNA_DUMP names it) with --skin into
~/.openagents/verse/private-build/metahuman-dump/, outside the repository: an OBJ per mesh
(centimetres), a skin file per mesh (joint index and weight pairs per vertex) and joints.txt
(name, parent, translation in cm, Euler XYZ rotation in degrees).

Then, for each mesh:
  - every joint folds onto the rig's 65 by name (aethyr/mannequin.coax mannequin_fold, ported
    here as fold()); a joint that folds to nothing gives its weight to its nearest ancestor
    that does (a face joint to the head, a finger helper to its finger);
  - the body is bound where the clothes were fitted (the DNA body with its arms lowered,
    rig-body-down), each kept joint carrying its turn from there to the rig's rest;
  - positions go to metres, Y up, +Z forward, the figure's left +x (the rig's axes);
  - weights keep the four heaviest after folding, summing to 255.

The head DNA's rig behaviour (OpenRigLogic's tables, dumped by dna_behavior beside dna_dump) gives
the pack a face rig (face_rig.py) the lab evaluates: expressions and blinks. Nothing this reads or writes
goes into git or is used to train AI models (the export's NO-TRAINING.txt).
"""
import math, os, subprocess
from pathlib import Path

import people
import face_rig
import pack_human

DUMP_DIR = Path.home() / ".openagents/verse/private-build/metahuman-dump"
OUTFITS = "~/Downloads/Modular Character Outfits - Fantasy[Standard]/Exports/glTF (Godot-Unreal)/Outfits"

# aethyr/mannequin.coax's names, in the rig's order.
NAMES = ["root", "pelvis", "spine_01", "spine_02", "spine_03", "neck_01", "Head",
         "clavicle_l", "upperarm_l", "lowerarm_l", "hand_l"]
for f in ("index", "middle", "pinky", "ring", "thumb"):
    NAMES += [f"{f}_01_l", f"{f}_02_l", f"{f}_03_l", f"{f}_04_leaf_l"]
NAMES += [n[:-2] + "_r" for n in NAMES[7:31]]
NAMES += ["thigh_l", "calf_l", "foot_l", "ball_l", "ball_leaf_l", "thigh_r", "calf_r", "foot_r", "ball_r", "ball_leaf_r"]
assert len(NAMES) == 65
LOWER = [n.lower() for n in NAMES]


def side(name):
    n = name.lower()
    return 1 if n.endswith("_l") or "_l_" in n else 2 if n.endswith("_r") or "_r_" in n else 0


def fold(name):
    """mannequin_fold: the rig's joint a body's joint is, or -1."""
    n = name.lower()
    if n in LOWER:
        return LOWER.index(n)
    if n in ("spine_04", "spine_05"):
        return LOWER.index("spine_03")
    if n == "neck_02":
        return LOWER.index("neck_01")
    sd = side(n)
    suffix = "_l" if sd == 1 else "_r" if sd == 2 else ""
    if "metacarpal" in n and suffix:
        return LOWER.index("hand" + suffix)
    for bone in ("clavicle", "upperarm", "lowerarm", "thigh", "calf", "foot", "hand"):
        if n.startswith(bone + "_") and suffix and n != bone + suffix:
            return LOWER.index(bone + suffix)
    return -1


# ---- maths: 3x3 rotations as row lists, column vectors

def euler_xyz(rx, ry, rz):
    d = math.pi / 180
    cx, sx, cy, sy, cz, sz = math.cos(rx * d), math.sin(rx * d), math.cos(ry * d), math.sin(ry * d), math.cos(rz * d), math.sin(rz * d)
    return [[cy * cz, cz * sx * sy - cx * sz, cx * cz * sy + sx * sz],
            [cy * sz, cx * cz + sx * sy * sz, -cz * sx + cx * sy * sz],
            [-sy, cy * sx, cx * cy]]
def mm(a, b): return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
def mv(a, v): return tuple(sum(a[i][k] * v[k] for k in range(3)) for i in range(3))
def tr(a): return [[a[j][i] for j in range(3)] for i in range(3)]
def sub(a, b): return tuple(x - y for x, y in zip(a, b))
def add(a, b): return tuple(x + y for x, y in zip(a, b))
def dot(a, b): return sum(x * y for x, y in zip(a, b))
def cross(a, b): return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
def unit(a):
    l = math.sqrt(dot(a, a)); return tuple(x / l for x in a) if l > 1e-12 else a
I3 = [[1, 0, 0], [0, 1, 0], [0, 0, 1]]


def arc(a, b):
    """The rotation taking unit a to unit b by the shortest arc (Rodrigues)."""
    a, b = unit(a), unit(b)
    c = dot(a, b)
    if c > 0.999999:
        return I3
    axis = cross(a, b)
    if c < -0.999999:
        axis = cross(a, (1, 0, 0)) if abs(a[0]) < 0.9 else cross(a, (0, 1, 0))
    k = unit(axis); s = math.sqrt(max(0.0, 1 - c * c)); t = 1 - c
    x, y, z = k
    return [[t * x * x + c, t * x * y - s * z, t * x * z + s * y],
            [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
            [t * x * z - s * y, t * y * z + s * x, t * z * z + c]]


class Joints:
    """A DNA's joints.txt: names, parents, world rotations and positions (cm, the DNA's space)."""
    def __init__(self, path):
        self.names, self.parent, self.R, self.p = [], [], [], []
        for line in Path(path).read_text().splitlines():
            w = line.split()
            name, par = w[0], int(w[1])
            t = tuple(float(x) for x in w[2:5]); r = euler_xyz(*(float(x) for x in w[5:8]))
            self.names.append(name); self.parent.append(par)
            if par < 0:
                self.R.append(r); self.p.append(t)
            else:
                self.R.append(mm(self.R[par], r)); self.p.append(add(self.p[par], mv(self.R[par], t)))
        self.kept = []
        for j, n in enumerate(self.names):
            k = fold(n); a = j
            while k < 0 and self.parent[a] >= 0:
                a = self.parent[a]; k = fold(self.names[a])
            self.kept.append(max(k, 0))
        self.of = {}
        for j, n in enumerate(self.names):
            if fold(n) >= 0 and n.lower() in LOWER and LOWER.index(n.lower()) not in self.of:
                self.of[LOWER.index(n.lower())] = j


def dna_space(joints):
    """The DNA's axes to the rig's (metres): found from the skeleton, not assumed: up is the
    pelvis to the head, the figure's left is the right thigh to the left, forward their cross."""
    up = unit(sub(joints.p[joints.of[LOWER.index("head")]], joints.p[joints.of[LOWER.index("pelvis")]]))
    left = unit(sub(joints.p[joints.of[LOWER.index("thigh_l")]], joints.p[joints.of[LOWER.index("thigh_r")]]))
    left = unit(sub(left, tuple(x * dot(left, up) for x in up)))
    fwd = cross(left, up)
    # rows: rig x (left), y (up), z (forward)
    return [list(left), list(up), list(fwd)]


def dump(dna, out):
    tool = os.environ.get("DNA_DUMP") or str(Path(dna).resolve().parents[4] / "src/openriglogic/build/dna_dump")
    out.mkdir(parents=True, exist_ok=True)
    if not (out / "joints.txt").exists() or (out / "joints.txt").stat().st_mtime < Path(dna).stat().st_mtime:
        subprocess.run([tool, str(dna), str(out), "--skin"], check=True, capture_output=True)
    # the face rig's tables (a head DNA): dna_behavior, built beside dna_dump (DNA_BEHAVIOR names it)
    behave = Path(os.environ.get("DNA_BEHAVIOR") or Path(tool).with_name("dna_behavior"))
    if "_Head" in Path(dna).name and behave.exists() and \
            (not (out / "targets.bin").exists() or (out / "targets.bin").stat().st_mtime < Path(dna).stat().st_mtime):
        subprocess.run([str(behave), str(dna), str(out / "behavior.txt"), str(out / "targets.bin")], check=True, capture_output=True)
    return out


def obj(path):
    vs, vts, vns, faces = [], [], [], []
    for line in Path(path).read_text().splitlines():
        if line.startswith("v "): vs.append(tuple(float(x) for x in line.split()[1:4]))
        elif line.startswith("vt "): vts.append(tuple(float(x) for x in line.split()[1:3]))
        elif line.startswith("vn "): vns.append(tuple(float(x) for x in line.split()[1:4]))
        elif line.startswith("f "):
            faces.append([tuple(int(x) - 1 for x in c.split("/")) for c in line.split()[1:]])
    return vs, vts, vns, faces


def skin_file(path):
    out = []
    for line in Path(path).read_text().splitlines():
        w = line.split(); n = int(w[0])
        out.append([(int(w[1 + 2 * k]), float(w[2 + 2 * k])) for k in range(n)])
    return out


def rig_rest():
    """The people rig's male rest: world positions (m) of its 65 joints."""
    sk = people.Skeleton(people.load(f"{OUTFITS}/Male_Peasant.gltf"))
    return [tuple(m[r][3] for r in range(3)) for m in sk.world], sk.parent


# Bones whose direction the T pose sets: (joint, the joint its bone points to).
def bone_ends(parent):
    child = {}
    for j, p in enumerate(parent):
        if p >= 0 and p not in child:
            child[p] = j
    # a limb's bone points along its chain, not to its first child (the hand's is the middle finger)
    for side in ("_l", "_r"):
        child[LOWER.index("hand" + side)] = LOWER.index("middle_01" + side)
        child[LOWER.index("clavicle" + side)] = LOWER.index("upperarm" + side)
        child[LOWER.index("foot" + side)] = LOWER.index("ball" + side)
    child[LOWER.index("spine_03")] = LOWER.index("neck_01")
    child[LOWER.index("pelvis")] = LOWER.index("spine_01")
    return child


# ---- the pose from MetaHuman's bind into the rig's rest

def t_pose(J, rest, rest_parent):
    """Per rig joint its world turn W (bind to T pose), its bind position p and posed position P
    (cm, the DNA's space), so a point skinned to rig joint k goes to W_k (x - p_k) + P_k. A joint
    the DNA lacks (the leaves) sits where the rig puts it past its parent, scaled to the parent's
    bone."""
    ends = bone_ends(rest_parent)
    W, p, P = [I3] * 65, [None] * 65, [None] * 65
    for k in range(65):
        par = rest_parent[k]
        if k in J.of:
            p[k] = J.p[J.of[k]]
        else:
            # a leaf: its parent's bone carried on by the rig's own length, in the DNA's centimetres
            pp = rest_parent[par]
            scale = 100.0
            if pp >= 0 and par in J.of and pp in J.of:
                rig_len = math.dist(rest[par], rest[pp]) or 1.0
                scale = math.dist(J.p[J.of[par]], J.p[J.of[pp]]) / rig_len
            p[k] = add(p[par], tuple(x * scale for x in mv(tr(W[par]), sub(rest[k], rest[par]))))
        if par < 0:
            P[k] = p[k]; W[k] = I3
        else:
            P[k] = add(P[par], mv(W[par], sub(p[k], p[par])))
            W[k] = W[par]
    # second pass: each limb bone turned onto the rig's rest direction, parents first
    W2, P2 = [I3] * 65, [None] * 65
    for k in range(65):
        par = rest_parent[k]
        P2[k] = p[k] if par < 0 else add(P2[par], mv(W2[par], sub(p[k], p[par])))
        base = I3 if par < 0 else W2[par]
        c = ends.get(k)
        if c is not None and aligned(k):
            now = mv(base, sub(p[c], p[k]))
            W2[k] = mm(arc(now, sub(rest[c], rest[k])), base)
        else:
            W2[k] = base
    return W2, p, P2


ALIGNED = ("clavicle", "upperarm", "lowerarm", "hand", "index", "middle", "ring", "pinky", "thumb", "thigh", "calf", "foot")
def aligned(k):
    return LOWER[k].split("_")[0] in ALIGNED


# ---- the meshes

# Each DNA mesh the pack draws: its material and its maps (the export's dcc/*/Maps). The eyes'
# shell (the lids' occlusion over the eye) and the eye edge (the wet layer along the lids) have no
# maps: they are blended over the eyes, shaded from lid coordinates (pack_human.lid_coords).
# Saliva and the tear ducts (cartilage) wait.
MESHES = {
    "body_lod0_mesh": ("skin", "Body_Basecolor.png", "Body_Normal.png", "Body_SRMF.png"),
    "head_lod0_mesh": ("skin", "Head_Basecolor.png", "Head_Normal.png", "Head_SRMF.png"),
    "teeth_lod0_mesh": ("teeth", "Teeth_Color.png", "Teeth_Normal.png", None),
    "eyeLeft_lod0_mesh": ("eye", "Eyes_Color.png", "Eyes_Normal.png", None),
    "eyeRight_lod0_mesh": ("eye", "Eyes_Color.png", "Eyes_Normal.png", None),
    "eyelashes_lod0_mesh": ("brow", "Eyelashes_Color.png", None, None),
    "eyeshell_lod0_mesh": ("occlusion", None, None, None),
    "eyeEdge_lod0_mesh": ("wet", None, None, None),
}


def maps_dir(export):
    found = sorted(export.glob("dcc/*/Maps"))
    if not found:
        raise SystemExit(f"metahuman: no dcc/*/Maps under {export}")
    return found[0]


def mesh(path, skin, J, flip_v, hide=None, posed=None):
    """One DNA mesh in the pack's bind pose, metres: vertices {p, n, uv, j, w} (one per distinct
    position, uv and normal a face corner uses) and triangles (its quads split). posed (positions
    and normals, the same mesh dumped in the clothes' pose) replaces the DNA's bind; a face whose
    positions are all in hide (the body under tight cloth) is left out."""
    vs, vts, vns, faces = obj(path)
    if posed:
        vs, vns = posed
    if len(skin) != len(vs):
        raise SystemExit(f"metahuman: {path.name}: {len(skin)} skin lines for {len(vs)} positions")
    # per position: its weights folded onto the rig's joints, the four heaviest
    folded = []
    for ws in skin:
        acc = {}
        for j, w in ws:
            k = J.kept[j]; acc[k] = acc.get(k, 0.0) + w
        top = sorted(acc.items(), key=lambda kv: -kv[1])[:4]
        while len(top) < 4:
            top.append((0, 0.0))
        folded.append(top)
    corner, verts, tris = {}, [], []
    for f in faces:
        if hide and all(pi in hide for (pi, _, _) in f):
            continue
        ids = []
        for (pi, ti, ni) in f:
            key = (pi, ti, ni)
            if key not in corner:
                corner[key] = len(verts)
                ws = folded[pi]
                x = vs[pi]; nn = vns[ni]
                u, v = vts[ti]
                verts.append({"p": tuple(c / 100.0 for c in x), "n": unit(nn), "uv": (u, 1.0 - v if flip_v else v), "pt": pi,
                              "j": [k if w > 0 else 0 for k, w in ws],
                              "w": people.quantize_weights([w for _, w in ws])})
            ids.append(corner[(pi, ti, ni)])
        for k in range(1, len(ids) - 1):
            tris.append((ids[0], ids[k], ids[k + 1]))
    return verts, tris


def iris_v(eye_path, eyes_image):
    """Whether the DNA's v runs up its images (as Maya's): the eye's front-most vertex is the iris's
    middle, the darkest disc of the eyes' colour map; of v and 1 - v, the one nearer it is right."""
    from PIL import Image
    vs, vts, vns, faces = obj(eye_path)
    front = max(range(len(vs)), key=lambda i: vs[i][2])
    uv = next(vts[t] for f in faces for (pi, t, ni) in f if pi == front)
    im = Image.open(eyes_image).convert("L").resize((128, 128)); px = im.load()
    def dark(u, v):
        x = min(127, max(0, int(u * 128))); y = min(127, max(0, int(v * 128)))
        return sum(px[min(127, max(0, x + dx)), min(127, max(0, y + dy))] for dx in (-2, 0, 2) for dy in (-2, 0, 2))
    return dark(uv[0], 1.0 - uv[1]) < dark(uv[0], uv[1])


def binds(M, W, p):
    """Each joint where the bind has it (metres), and its turn from there to the rig's rest (the T
    pose), x y z w: a point at x in the bind goes to W_k R_k^T (x - M_k p_k) + P_k at rest."""
    return [(*(c / 100.0 for c in add(mv(M[k][0], p[k]), M[k][1])), *quat(mm(W[k], tr(M[k][0])))) for k in range(65)]


def body_female(P):
    """Whether the people rig's female body is nearer these proportions (its T pose) than the male:
    that body plays its clips."""
    def off(sk):
        return sum(math.dist(a, tuple(c / 100.0 for c in b)) for a, b in zip(sk, P))
    rest, _ = rig_rest()
    female = people.Skeleton(people.load(f"{OUTFITS}/Female_Peasant.gltf"))
    return off([tuple(m[r][3] for r in range(3)) for m in female.world]) < off(rest)


def build(pack, export):
    dna = sorted((export / "dna").glob("*_Body.dna")), sorted((export / "dna").glob("*_Head.dna"))
    if not dna[0] or not dna[1]:
        raise SystemExit(f"metahuman: no *_Body.dna and *_Head.dna under {export}/dna")
    body = dump(dna[0][0], DUMP_DIR / "body"); head = dump(dna[1][0], DUMP_DIR / "head")
    maps = maps_dir(export)
    JB, JH = Joints(body / "joints.txt"), Joints(head / "joints.txt")
    S = dna_space(JB)
    if any(abs(S[i][i]) < 0.99 for i in range(3)):
        raise SystemExit("metahuman: the DNA's axes aren't the rig's (y up, z forward)")
    rest, rest_parent = rig_rest()
    W, p, P = t_pose(JB, rest, rest_parent)
    flip_v = iris_v(head / "eyeLeft_lod0_mesh.obj", maps / "Eyes_Color.png")
    portrait = export.parent / "latest/mh.neutral.glb"
    skin = skin_maps(portrait, maps) if portrait.exists() else {}
    # the bind pose: the clothes' own (the body with its arms lowered), each joint's motion to it
    down = arms_down(body, JB, export.parent / "rig-body-down/body_lod0_mesh.obj", rest_parent)
    M = down["M"]
    covered = set()
    if portrait.exists():
        covered = dressed(pack, [portrait] + [q for q in LOWER_GARMENTS(export) if q.exists()],
                          down, (head, JH), W, p, P)
    for name, (material, base, normal, rough) in MESHES.items():
        src = body if name.startswith("body") else head
        J = JB if src is body else JH
        path = src / f"{name}.obj"
        if not path.exists():
            continue
        verts, tris = mesh(path, skin_file(src / f"{name}.skin.txt"), J, flip_v,
                           hide=covered if src is body else None,
                           posed=(down["x"], down["vn"]) if src is body else None)
        layers = skin.get(name) or (maps / base if base else None, maps / normal if normal else None, maps / rough if rough else None)
        if pack_human.MATERIALS[material] in pack_human.BLENDED:
            verts = pack_human.lid_coords(verts, tris)
        pack.piece(name.replace("_lod0_mesh", ""), verts, tris, material, *layers)
        if src is head and name in face_rig.FACE_MESHES:
            pack.pieces[-1]["face"] = name
    face(pack, head)
    pack.binds = binds(M, W, p)
    pack.tints.update({"skin": (1.0, 1.0, 1.0, 1.0), "eye": (1.0, 1.0, 1.0, 0.25), "teeth": (1.0, 1.0, 1.0, 0.5),
                       "brow": (1.0, 1.0, 1.0, 0.6), "hair": (1.0, 1.0, 1.0, 0.55)})
    pack.tints.setdefault("cloth", (1.0, 1.0, 1.0, 1.0))
    pack.tints.update(pack_human.EYE_TINTS)
    pack.body = "female" if body_female(P) else "male"
    return {"flip_v": flip_v}


# ---- the face rig (face_rig.py): the DNA's controls, joints and blend shapes for the lab

def face(pack, head):
    """Gives the pack its face rig: the head DNA's rig behaviour (dna_behavior, run by dump()) and
    the face meshes' points, normals and skins, each face vertex knowing its point."""
    behavior, tpath = head / "behavior.txt", head / "targets.bin"
    if not behavior.exists() or not tpath.exists():
        print("metahuman: no face rig (dna_behavior's behavior.txt and targets.bin are missing)")
        return
    bh = face_rig.Behavior(behavior)
    tg = face_rig.targets(tpath, None)
    meshes, first, at = [], {}, 0
    for name in face_rig.FACE_MESHES:
        vs, _, vns, fs = obj(head / f"{name}.obj")
        meshes.append((name, vs, position_normals(vs, vns, fs), skin_file(head / f"{name}.skin.txt")))
        first[name] = at; at += len(vs)
    index = {n: i for i, n in enumerate(bh.meshes)}
    def block(pieces):
        points = [first[p["face"]] + v["pt"] for p in pieces for v in p["verts"]]
        return face_rig.pack(bh, tg, meshes, index, points)
    pack.face = block
    # the reference's reach, as numbers (an expression that moves nothing is a broken rig)
    vs, _, _, _ = obj(head / "head_lod0_mesh.obj"); sk = skin_file(head / "head_lod0_mesh.skin.txt")
    for name in ("blink", "jaw"):
        raw = {bh.control(k): v for k, v in face_rig.EXPRESSIONS[name].items()}
        q = face_rig.skinned(bh, tg, raw, vs, sk, {index["head_lod0_mesh"]: 0})
        far = max(math.dist(a, b) for a, b in zip(q, vs))
        print(f"face rig: {name} moves the head's points up to {far:.2f} cm")
        if far < 0.5:
            raise SystemExit(f"metahuman: the face rig's {name} barely moves the face")


# ---- the portrait's skin maps, groom and clothes

# The portrait (the export's latest/mh.neutral.glb, metres, the DNA's bind space) and the lower
# garments beside it (the film's world/*_lower.glb, the same space): hair cards and eyebrow cards
# ride the head; the clothes were fitted to the body with its arms lowered (the export's
# rig-body-down, the DNA body posed by dna_dump), so each one is skinned against that body and
# moved back to the bind pose before it goes to the T pose with the body. Their own head, eyes
# and teeth are the DNA's already; the strand-dense hair (1.5M vertices), the blended layers
# (cornea, wet edges) and the skin patch at the waist wait.
GROOM = ("groom_Hair_S_UpdoBuns", "groom_Eyebrows_M_Natural")
SKIP = ("draconia_skin", "mh_eye", "mh_cornea", "mh_wet", "mh_teeth", "mh_lashes", "draconia_hair", "draconia_brow",
        "draconia_body_skin")


def LOWER_GARMENTS(export):
    return sorted((export.parent.parent / "world").glob("*_lower.glb"))


def glb_image(g, name, out):
    """A glb's embedded image by name, saved once under the private dump (a path pack_human reads)."""
    from PIL import Image
    import io
    for im in g["images"]:
        if im.get("name") == name:
            path = out / f"{name}.png"
            if not path.exists():
                bv = g["bufferViews"][im["bufferView"]]; o = bv.get("byteOffset", 0)
                path.parent.mkdir(parents=True, exist_ok=True)
                Image.open(io.BytesIO(g["bins"][0][o:o + bv["byteLength"]])).save(path)
            return path
    return None


def skin_maps(portrait, maps):
    """The skin's layers as the film shot them. The portrait's head maps (base, normal, and an
    occlusion-roughness-metal map, glTF's) are the export's Head_Basecolor toned by a per-channel
    line; the body gets the same line, so neck and face meet. The export's SRMF maps are empty, so
    the body's roughness is the head's mean, and the occlusion rides the base's alpha (scene_fs
    reads it for skin)."""
    from PIL import Image
    out = DUMP_DIR / "portrait"
    g = people.load(str(portrait))
    base, orm, normal = (glb_image(g, n, out) for n in ("mh_skin_base", "mh_skin_orm", "mh_skin_normal"))
    if not (base and orm and normal):
        return {}
    a = Image.open(base).convert("RGB").resize((128, 128)); h = Image.open(maps / "Head_Basecolor.png").convert("RGB").resize((128, 128))
    line = []
    for c in range(3):
        xs = list(h.split()[c].tobytes()); ys = list(a.split()[c].tobytes()); n = len(xs)
        mx, my = sum(xs) / n, sum(ys) / n
        k = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / max(sum((x - mx) ** 2 for x in xs), 1e-9)
        line.append((k, my - k * mx))
    head_base = out / "head_base_ao.png"
    o = Image.open(orm).convert("RGB")
    if not head_base.exists():
        Image.merge("RGBA", (*Image.open(base).convert("RGB").split(), o.split()[0])).save(head_base)
    body_base = out / "body_base_toned.png"
    if not body_base.exists():
        ch = Image.open(maps / "Body_Basecolor.png").convert("RGB").split()
        ch = [c.point(lambda x, k=k, b=b: max(0, min(255, round(k * x + b)))) for c, (k, b) in zip(ch, line)]
        Image.merge("RGB", ch).save(body_base)
    g_mean = round(sum(o.split()[1].resize((64, 64)).tobytes()) / 4096)
    body_rough = out / "body_rough.png"
    if not body_rough.exists():
        Image.new("RGB", (64, 64), (255, g_mean, 0)).save(body_rough)
    return {"head_lod0_mesh": (head_base, normal, orm),
            "body_lod0_mesh": (body_base, maps / "Body_Normal.png", body_rough)}


def inv3(a):
    (p, q, r), (s, t, u), (v, w, x) = a
    d = p * (t * x - u * w) - q * (s * x - u * v) + r * (s * w - t * v)
    if abs(d) < 1e-12:
        return I3
    return [[(t * x - u * w) / d, (r * w - q * x) / d, (q * u - r * t) / d],
            [(u * v - s * x) / d, (p * x - r * v) / d, (r * s - p * u) / d],
            [(s * w - t * v) / d, (q * v - p * w) / d, (p * t - q * s) / d]]


def horn(A, B):
    """The rigid motion (R, t) taking points A onto B best, by Horn's quaternion method."""
    n = len(A)
    ca = tuple(sum(a[i] for a in A) / n for i in range(3)); cb = tuple(sum(b[i] for b in B) / n for i in range(3))
    S = [[0.0] * 3 for _ in range(3)]
    for a, b in zip(A, B):
        a = sub(a, ca); b = sub(b, cb)
        for i in range(3):
            for j in range(3):
                S[i][j] += a[i] * b[j]
    (xx, xy, xz), (yx, yy, yz), (zx, zy, zz) = S
    N = [[xx + yy + zz, yz - zy, zx - xz, xy - yx],
         [yz - zy, xx - yy - zz, xy + yx, zx + xz],
         [zx - xz, xy + yx, -xx + yy - zz, yz + zy],
         [xy - yx, zx + xz, yz + zy, -xx - yy + zz]]
    c = sum(abs(v) for row in N for v in row) + 1.0
    q = [1.0, 0.0, 0.0, 0.0]
    for _ in range(200):
        q2 = [sum((N[i][j] + (c if i == j else 0.0)) * q[j] for j in range(4)) for i in range(4)]
        l = math.sqrt(sum(v * v for v in q2)); q = [v / l for v in q2]
    w, x, y, z = q
    R = [[1 - 2 * (y * y + z * z), 2 * (x * y - w * z), 2 * (x * z + w * y)],
         [2 * (x * y + w * z), 1 - 2 * (x * x + z * z), 2 * (y * z - w * x)],
         [2 * (x * z - w * y), 2 * (y * z + w * x), 1 - 2 * (x * x + y * y)]]
    return R, sub(cb, mv(R, ca))


class Near:
    """Points (cm) in 2 cm cells, for radius searches."""
    def __init__(self, xs, cell=2.0):
        self.cell, self.xs, self.cells = cell, xs, {}
        for i, x in enumerate(xs):
            self.cells.setdefault(tuple(int(math.floor(c / cell)) for c in x), []).append(i)
    def within(self, x, r):
        c = tuple(int(math.floor(v / self.cell)) for v in x); k = int(math.ceil(r / self.cell))
        out = []
        for dx in range(-k, k + 1):
            for dy in range(-k, k + 1):
                for dz in range(-k, k + 1):
                    for i in self.cells.get((c[0] + dx, c[1] + dy, c[2] + dz), ()):
                        d = math.dist(self.xs[i], x)
                        if d <= r:
                            out.append((d, i))
        return out


def folded_weights(skin, J):
    out = []
    for ws in skin:
        acc = {}
        for j, w in ws:
            k = J.kept[j]; acc[k] = acc.get(k, 0.0) + w
        t = sum(acc.values()) or 1.0
        out.append({k: w / t for k, w in acc.items()})
    return out


def position_normals(vs, vns, faces):
    n = [(0.0, 0.0, 0.0)] * len(vs)
    for f in faces:
        for (pi, _, ni) in f:
            n[pi] = add(n[pi], vns[ni])
    return [unit(x) for x in n]


def quat(R):
    """A rotation matrix's unit quaternion (x, y, z, w)."""
    t = R[0][0] + R[1][1] + R[2][2]
    if t > 0:
        w = math.sqrt(1 + t) * 2; q = ((R[2][1] - R[1][2]) / w, (R[0][2] - R[2][0]) / w, (R[1][0] - R[0][1]) / w, w / 4)
    elif R[0][0] > R[1][1] and R[0][0] > R[2][2]:
        w = math.sqrt(1 + R[0][0] - R[1][1] - R[2][2]) * 2; q = (w / 4, (R[0][1] + R[1][0]) / w, (R[0][2] + R[2][0]) / w, (R[2][1] - R[1][2]) / w)
    elif R[1][1] > R[2][2]:
        w = math.sqrt(1 + R[1][1] - R[0][0] - R[2][2]) * 2; q = ((R[0][1] + R[1][0]) / w, w / 4, (R[1][2] + R[2][1]) / w, (R[0][2] - R[2][0]) / w)
    else:
        w = math.sqrt(1 + R[2][2] - R[0][0] - R[1][1]) * 2; q = ((R[0][2] + R[2][0]) / w, (R[1][2] + R[2][1]) / w, w / 4, (R[1][0] - R[0][1]) / w)
    l = math.sqrt(sum(c * c for c in q)); return tuple(c / l for c in q)


def arms_down(body, JB, down_path, rest_parent):
    """The body as the clothes were fitted to it (dna_dump's arms-down pose; the bind itself when
    the export has none): positions, corner normals, per-position normals, folded weights, and each
    rig joint's rigid motion from the DNA's bind to it, fitted (Horn) to the vertices it nearly owns
    (a joint with too few, or one that barely moves, follows its parent or stays put)."""
    bdir = body
    vb, _, _, _ = obj(bdir / "body_lod0_mesh.obj")
    vd, _, vnd, fd = obj(down_path) if down_path.exists() else obj(bdir / "body_lod0_mesh.obj")
    if len(vd) != len(vb):
        raise SystemExit("metahuman: the arms-down body doesn't match the bind body's vertices")
    wb = folded_weights(skin_file(bdir / "body_lod0_mesh.skin.txt"), JB)
    pairs = {}
    for x, y, w in zip(vb, vd, wb):
        k, top = max(w.items(), key=lambda kv: kv[1])
        if top >= 0.9:
            pairs.setdefault(k, ([], []))
            if len(pairs[k][0]) < 600:
                pairs[k][0].append(x); pairs[k][1].append(y)
    M = [None] * 65
    for k in range(65):
        if k in pairs and len(pairs[k][0]) >= 6:
            R, t = horn(*pairs[k])
            moved = max(math.dist(mv(R, x), sub(y, t)) for x, y in zip(*pairs[k]))
            still = max(math.dist(x, y) for x, y in zip(*pairs[k]))
            M[k] = (I3, (0.0, 0.0, 0.0)) if still < 0.05 else (R, t)
        else:
            M[k] = M[rest_parent[k]] if rest_parent[k] >= 0 else (I3, (0.0, 0.0, 0.0))
    return {"x": vd, "vn": vnd, "n": position_normals(vd, vnd, fd), "w": wb, "M": M}


def dressed(pack, sources, down, head, W, p, P):
    """Packs each source's grooms and garments, as authored (the bind pose is theirs); returns the
    body positions tight cloth covers."""
    (hdir, JH) = head
    vd, nd, wb, M = down["x"], down["n"], down["w"], down["M"]
    vh, _, vnh, fh = obj(hdir / "head_lod0_mesh.obj")
    wh = folded_weights(skin_file(hdir / "head_lod0_mesh.skin.txt"), JH)
    nh = position_normals(vh, vnh, fh)
    # the reference surface: the arms-down body and the head (which the pose leaves alone)
    poses = clip_poses(binds(M, W, p), "female" if body_female(P) else "male")
    ref_x = vd + vh; ref_n = nd + nh; ref_w = wb + wh
    # the arm bones where the clothes' pose has them (cm)
    bone = lambda n: add(mv(M[LOWER.index(n)][0], p[LOWER.index(n)]), M[LOWER.index(n)][1])
    arm_bones = {sd: [(bone("upperarm_" + sd), bone("lowerarm_" + sd)), (bone("lowerarm_" + sd), bone("hand_" + sd))] for sd in "lr"}
    # which arm a body point belongs to (most of its weight on that arm's bones or hand)
    ref_arm = []
    for w in ref_w:
        a = {"l": 0.0, "r": 0.0}
        for k, v in w.items():
            sd = limb_side(k)
            if sd:
                a[sd] += v
        ref_arm.append("l" if a["l"] > 0.5 else "r" if a["r"] > 0.5 else None)
    near = Near(ref_x)
    head_joint = LOWER.index("head")
    cloth_ids, packed, wanted, garment_x, garment_w = {}, [], [], [], []
    report = []
    for src in sources:
        g = people.load(str(src))
        out = DUMP_DIR / "portrait"
        def image(t):
            if not t:
                return None
            im = g["images"][g["textures"][t["index"]]["source"]]
            return glb_image(g, im.get("name", "image"), out)
        for node in g["nodes"]:
            if "mesh" not in node:
                continue
            prims = []
            for pr in g["meshes"][node["mesh"]]["primitives"]:
                mat = g["materials"][pr["material"]]
                if mat["name"] in SKIP:
                    continue
                prims.append((pr, mat))
            if not prims:
                continue
            wanted.append(node["name"])
            groom = prims[0][1]["name"] in GROOM
            # the node's vertices welded by position, so weights smooth across its uv seams
            data, key_of, keys = [], {}, []
            for pr, mat in prims:
                a = pr["attributes"]
                pos = [tuple(c * 100.0 for c in x) for x in g["acc"](a["POSITION"])]
                data.append((pr, mat, pos, g["acc"](a["NORMAL"]), g["acc"](a["TEXCOORD_0"]), g["acc"](pr["indices"])))
                ks = []
                for x in pos:
                    q = tuple(round(c * 20) for c in x)
                    if q not in key_of:
                        key_of[q] = len(keys); keys.append(x)
                    ks.append(key_of[q])
                keys_of_prim = ks
                data[-1] = data[-1] + (keys_of_prim,)
            gap = []
            if groom:
                kw = [{head_joint: 1.0} for _ in keys]
            else:
                # per welded vertex: weights from the body points near it (all of them, and those
                # off the arms), and the body part it lies outside of (its sleeve's arm or none)
                kw, kw_core, cand = [], [], []
                for x in keys:
                    found, r = [], 4.0
                    while not found and r <= 64.0:
                        found = near.within(x, r); r *= 2
                    if not found:
                        gap.append(99.0); kw.append({LOWER.index("spine_03"): 1.0}); kw_core.append(None); cand.append(None); continue
                    found.sort()
                    gap.append(found[0][0])
                    sigma = max(1.0, 1.5 * found[0][0])
                    acc, core = {}, {}
                    for d, i in found[:24]:
                        f = math.exp(-(d / sigma) ** 2)
                        if dot(sub(x, ref_x[i]), ref_n[i]) < -0.3:
                            f *= 0.25
                        for k, w in ref_w[i].items():
                            acc[k] = acc.get(k, 0.0) + f * w
                        if ref_arm[i] is None:
                            for k, w in ref_w[i].items():
                                core[k] = core.get(k, 0.0) + f * w
                    t = sum(acc.values()) or 1.0
                    kw.append({k: w / t for k, w in acc.items()})
                    if sum(core.values()) < 1e-6:
                        # by a hand: the nearest body points off the arms, farther out
                        off = sorted((d, i) for d, i in near.within(x, 16.0) if ref_arm[i] is None)[:24]
                        for d, i in off:
                            f = math.exp(-(d / max(1.0, 1.5 * off[0][0])) ** 2)
                            for k, w in ref_w[i].items():
                                core[k] = core.get(k, 0.0) + f * w
                    t = sum(core.values())
                    kw_core.append({k: w / t for k, w in core.items()} if t > 1e-6 else None)
                    # the nearest body point the cloth lies outside of: a sleeve lies outside the
                    # arm, a robe's side between arm and trunk lies outside the trunk
                    out_of = None
                    for d, i in found[:48]:
                        if d < 0.3 or dot(sub(x, ref_x[i]), ref_n[i]) > 0.2 * d:
                            out_of = i; break   # (cloth on the skin itself, as tights, is outside it)
                    sd = ref_arm[out_of if out_of is not None else found[0][1]]
                    # and a sleeve ends at the wrist: its foot falls on the upper arm or forearm (or the cuff just past)
                    if sd and not any(-0.05 <= seg(x, a, b)[1] <= 1.15 for a, b in arm_bones[sd]):
                        sd = None
                    cand.append(sd)
                nbr = [set() for _ in keys]
                for (_, _, _, _, _, idx, ks) in data:
                    for t in range(0, len(idx), 3):
                        a, b, c = ks[idx[t]], ks[idx[t + 1]], ks[idx[t + 2]]
                        nbr[a].update((b, c)); nbr[b].update((a, c)); nbr[c].update((a, b))
                kw = sleeves(kw, kw_core, cand, nbr, keys, arm_bones)
                # loose cloth (most of it off the body: a robe, a skirt) hangs from the hips all the
                # way down, half or more from the pelvis (a long hem between the legs can't follow both
                # thighs), and blends its weights over a few centimetres of space, so where it
                # parts from a hanging arm or between the legs it stretches over that span instead
                # of tearing at one edge; closer cloth hangs from the hip only where it is off the
                # body
                gs = sorted(gap)
                print(f"gap {node['name']}: cm off the body 25/50/75/95% " + " ".join(f"{gs[int(len(gs) * q)]:.1f}" for q in (0.25, 0.5, 0.75, 0.95)))
                close = gs[len(gs) // 2] <= 1.0
                if not close:
                    kw = spread(hanging(loose(kw, [99.0] * len(kw)), keys), keys, sigma=6.0)
                else:
                    kw = loose(kw, gap)
                    # a layer over clothes (a sweater over a shirt) blends over a little space too,
                    # where its sleeve was fitted against its side
                    if gs[len(gs) // 2] > 0.5:
                        kw = spread(kw, keys, sigma=3.0, reach=2)
                # smoothing over the cloth's own neighbours
                for _ in range(6):
                    nxt = []
                    for i, w in enumerate(kw):
                        if not nbr[i]:
                            nxt.append(w); continue
                        acc = {k: 0.5 * v for k, v in w.items()}
                        f = 0.5 / len(nbr[i])
                        for j in nbr[i]:
                            for k, v in kw[j].items():
                                acc[k] = acc.get(k, 0.0) + f * v
                        nxt.append(acc)
                    kw = nxt
                # close cloth (a sweater) spreads a raised arm's pull over the band where its
                # sleeve meets its side; loose cloth (a robe) is blended over 6 cm already, and
                # more there spikes its edges in a walk
                if close:
                    kw = armpit(kw, nbr)
                # a small piece (a tie, a collar's fall, a trim, a shirt's front) moves as one: its
                # vertices all take the piece's mean weights, so it can't stretch
                lo = [min(x[i] for x in keys) for i in range(3)]; hi = [max(x[i] for x in keys) for i in range(3)]
                if max(h - l for h, l in zip(hi, lo)) < 40.0:
                    acc = {}
                    for w in kw:
                        for k, v in w.items():
                            acc[k] = acc.get(k, 0.0) + v / len(kw)
                    kw = [dict(acc) for _ in kw]
            for (pr, mat, pos, nor, uv, idx, ks) in data:
                pbr = mat.get("pbrMetallicRoughness", {})
                if groom:
                    material = "brow" if "Eyebrows" in mat["name"] else "hair"
                else:
                    rgb = tuple(pbr.get("baseColorFactor", (1, 1, 1, 1))[:3])
                    key = (rgb, round(pbr.get("roughnessFactor", 0.8), 3))
                    if key not in cloth_ids:
                        cloth_ids[key] = f"cloth{len(cloth_ids)}"
                        pack.tints[cloth_ids[key]] = (*rgb, key[1])
                    material = cloth_ids[key]
                verts, fw = [], []
                for x, nn, t, kk in zip(pos, nor, uv, ks):
                    w = sorted(kw[kk].items(), key=lambda kv: -kv[1])[:4]
                    tot = sum(v for _, v in w) or 1.0
                    w = [(k, v / tot) for k, v in w]
                    ws = (w + [(0, 0.0)] * 4)[:4]
                    verts.append({"p": tuple(c / 100.0 for c in x), "n": unit(nn), "uv": tuple(t),
                                  "j": [k if v > 0 else 0 for k, v in ws], "w": people.quantize_weights([v for _, v in ws])})
                    fw.append(w)
                    if not groom:
                        garment_x.append(x); garment_w.append(dict(w))
                tris = [tuple(idx[k:k + 3]) for k in range(0, len(idx), 3)]
                if not groom:
                    report.append((node["name"], edge_check(pos, fw, tris, poses)))
                pack.piece(node["name"], verts, tris, material, image(pbr.get("baseColorTexture")),
                           image(mat.get("normalTexture")), image(pbr.get("metallicRoughnessTexture")))
                packed.append(node["name"])
    # the checks a render can't be looked at for
    missing = sorted(set(wanted) - set(packed))
    print("garments: source", sorted(set(wanted)), "packed", sorted(set(packed)), "missing", missing)
    if missing:
        raise SystemExit(f"metahuman: garments not packed: {missing}")
    bad = []
    for name, res in report:
        line = "  ".join(f"{k} {v[0]:.2%}/{v[1]:.0f}x {v[2]:.2%}/{v[3]:.1f}cm" for k, v in res.items())
        print(f"edges {name} (share past 1.5x / worst ratio, share past 1.5x and 1 cm / most grown): {line}")
        # a spike is an edge grown by half again and by centimetres (a tiny edge doubled is not one)
        if any(v[2] > 0.005 or v[3] > 4.0 for k, v in res.items() if k.startswith(("idle", "walk"))):
            bad.append(name)
    if bad:
        raise SystemExit(f"metahuman: torn cloth (edge outliers) in {bad}")
    # the body under tight cloth: positions with a garment within 2 cm outward that moves with them
    # (most of their weights shared), so a hand resting on the hip isn't hidden by the hip's cloth
    gnear = Near(garment_x)
    covered = set()
    for i, (x, n) in enumerate(zip(vd, nd)):
        for d, j in gnear.within(x, 2.0):
            if dot(sub(garment_x[j], x), n) > -0.2 and \
                    sum(min(v, garment_w[j].get(k, 0.0)) for k, v in wb[i].items()) >= 0.5:
                covered.add(i); break
    hands = [i for i, w in enumerate(wb) if sum(v for k, v in w.items() if LOWER[k].split("_")[0] in HAND) > 0.5]
    print(f"body positions under tight cloth: {len(covered)} of {len(vd)}; "
          f"hand positions shown: {sum(i not in covered for i in hands)} of {len(hands)}")
    if hands and sum(i not in covered for i in hands) < 0.9 * len(hands):
        raise SystemExit("metahuman: the hands are hidden under cloth")
    return covered


# Cloth follows the arm only along a sleeve: a garment point is a sleeve's when the nearest body
# point it lies outside of is an arm's (a robe's side between a hanging arm and the trunk lies
# outside the trunk) and it is on a big connected run of such points; hands and fingers give their
# weight to the forearm (a sleeve ends at the wrist). Off a sleeve, a point takes its weights from
# the body points off the arms only.
ARM = ("upperarm", "lowerarm")
HAND = ("hand", "index", "middle", "ring", "pinky", "thumb")
def arm_side(k):
    n = LOWER[k]
    if n.split("_")[0] in ARM:
        return n[-1]
    return None


def limb_side(k):
    n = LOWER[k]
    if n.split("_")[0] in ARM + HAND:
        return n[-1]
    return None


def armpit(kw, nbr, iters=30, rings=2):
    """Where a garment's weights mix the arm with the trunk (a sleeve meeting the side), and two
    rings round that, its weights smoothed over its neighbours 30 times more: a raised arm (the
    slash, the spell) stretches the cloth across that whole band instead of along the seam. The
    sleeve's own points stay with the arm (in the clips, 95% within about a centimetre)."""
    arm = {k for k, n in enumerate(LOWER) if n.split("_")[0] in ARM + HAND}
    share = lambda w: sum(v for k, v in w.items() if k in arm) / max(sum(w.values()), 1e-9)
    band = {i for i, w in enumerate(kw) if 0.02 < share(w) < 0.98}
    for _ in range(rings):
        band |= {j for i in band for j in nbr[i]}
    for _ in range(iters):
        nxt = list(kw)
        for i in band:
            if not nbr[i]:
                continue
            acc = {k: 0.5 * v for k, v in kw[i].items()}
            f = 0.5 / len(nbr[i])
            for j in nbr[i]:
                for k, v in kw[j].items():
                    acc[k] = acc.get(k, 0.0) + f * v
            nxt[i] = acc
        kw = nxt
    return kw


def loose(kw, gap):
    """Cloth well off the body (past 3 cm: a robe's or a skirt's hem) hangs from the hip, not the
    knee: its calf, foot and toe weight goes to the thigh."""
    up = {}
    for k, n in enumerate(LOWER):
        for part in ("calf", "foot", "ball"):
            if n.startswith(part + "_"):
                up[k] = LOWER.index("thigh_" + n[-1])
    out = []
    for w, g in zip(kw, gap):
        if g <= 3.0:
            out.append(w); continue
        acc = {}
        for k, v in w.items():
            k = up.get(k, k); acc[k] = acc.get(k, 0.0) + v
        out.append(acc)
    return out


def spread(kw, keys, cell=2.0, sigma=4.0, reach=3):
    """Weights blended in space: averaged per 2 cm cell, the cells smoothed with a Gaussian, and
    each point given its cell field's value at it (trilinear)."""
    cells = {}
    for x, w in zip(keys, kw):
        c = tuple(int(math.floor(v / cell)) for v in x)
        acc = cells.setdefault(c, [0, {}]); acc[0] += 1
        for k, v in w.items():
            acc[1][k] = acc[1].get(k, 0.0) + v
    mean = {c: {k: v / n for k, v in w.items()} for c, (n, w) in cells.items()}
    smooth = {}
    need = set()
    for c in mean:
        for d in range(27):
            need.add((c[0] + d % 3 - 1, c[1] + d // 3 % 3 - 1, c[2] + d // 9 - 1))
    for c in need:
        acc, tot = {}, 0.0
        for dx in range(-reach, reach + 1):
            for dy in range(-reach, reach + 1):
                for dz in range(-reach, reach + 1):
                    m = mean.get((c[0] + dx, c[1] + dy, c[2] + dz))
                    if m is None:
                        continue
                    f = math.exp(-((dx * dx + dy * dy + dz * dz) * cell * cell) / (sigma * sigma))
                    tot += f
                    for k, v in m.items():
                        acc[k] = acc.get(k, 0.0) + f * v
        if tot > 0:
            smooth[c] = {k: v / tot for k, v in acc.items()}
    out = []
    for x, w in zip(keys, kw):
        g = [v / cell - 0.5 for v in x]; b = [int(math.floor(v)) for v in g]; f = [v - i for v, i in zip(g, b)]
        acc, tot = {}, 0.0
        for d in range(8):
            o = (d & 1, d >> 1 & 1, d >> 2 & 1)
            m = smooth.get((b[0] + o[0], b[1] + o[1], b[2] + o[2]))
            if m is None:
                continue
            a = 1.0
            for i in range(3):
                a *= f[i] if o[i] else 1.0 - f[i]
            tot += a
            for k, v in m.items():
                acc[k] = acc.get(k, 0.0) + a * v
        out.append({k: v / tot for k, v in acc.items()} if tot > 1e-9 else w)
    return out


def hanging(kw, keys):
    """Half of each point's thigh weight to the pelvis, more on a long hem (up to 85% at the
    ankles: there a step parts the thighs by tens of centimetres)."""
    pelvis = LOWER.index("pelvis"); thighs = {LOWER.index("thigh_l"), LOWER.index("thigh_r")}
    out = []
    for w, x in zip(kw, keys):
        share = min(0.85, 0.5 + max(0.0, 70.0 - x[1]) / 100.0)
        acc = {}
        for k, v in w.items():
            if k in thighs:
                acc[pelvis] = acc.get(pelvis, 0.0) + share * v; v *= 1.0 - share
            acc[k] = acc.get(k, 0.0) + v
        out.append(acc)
    return out


def seg(x, a, b):
    """A point's distance to the segment a-b, and how far along it its foot falls (0 at a, 1 at b)."""
    ab = sub(b, a); l2 = dot(ab, ab) or 1e-9
    t = dot(sub(x, a), ab) / l2
    f = add(a, tuple(c * min(max(t, 0.0), 1.0) for c in ab))
    return math.dist(x, f), t


def wraps(xs, a, b):
    """How many of 12 sectors around the bone a-b the points along its middle fill. A sleeve fills
    them all around the upper arm; a robe's front by a hand fills a few, and tights where a wrist
    rests on the hip none there."""
    axis = unit(sub(b, a)); e1 = cross(axis, (0.0, 0.0, 1.0))
    if math.sqrt(dot(e1, e1)) < 0.1:
        e1 = cross(axis, (1.0, 0.0, 0.0))
    e1 = unit(e1); e2 = cross(axis, e1)
    hit = set()
    for x in xs:
        _, t = seg(x, a, b)
        if 0.15 <= t <= 0.85:
            v = sub(x, add(a, tuple(c * t for c in sub(b, a))))
            hit.add(int((math.atan2(dot(v, e2), dot(v, e1)) + math.pi) / (2 * math.pi) * 12) % 12)
    return len(hit)


def sleeves(kw, kw_core, cand, nbr, keys, arm_bones):
    hand = {k: LOWER.index("lowerarm_" + n[-1]) for k, n in enumerate(LOWER) if n.split("_")[0] in HAND}
    def folded(w):
        acc = {}
        for k, v in w.items():
            k = hand.get(k, k); acc[k] = acc.get(k, 0.0) + v
        return acc
    side = list(cand)
    seen = [False] * len(kw)
    keep = max(200, len(kw) // 50)
    for i in range(len(kw)):
        if seen[i] or side[i] is None:
            continue
        run, stack = [], [i]; seen[i] = True
        while stack:
            j = stack.pop(); run.append(j)
            for k in nbr[j]:
                if not seen[k] and side[k] == side[i]:
                    seen[k] = True; stack.append(k)
        if len(run) < keep or wraps([keys[j] for j in run], *arm_bones[side[i]][0]) < 10:
            for j in run:
                side[j] = None
    # a sleeve never follows the hips or legs (a cuff by clasped hands lies over the thigh)
    legs = {k for k, n in enumerate(LOWER) if n == "pelvis" or n.split("_")[0] in ("thigh", "calf", "foot", "ball")}
    out = []
    for j, w in enumerate(kw):
        src = folded(w if side[j] is not None or kw_core[j] is None else kw_core[j])
        if side[j] is not None:
            src = {k: v for k, v in src.items() if k not in legs} or {LOWER.index("lowerarm_" + side[j]): 1.0}
        core = {k: v for k, v in src.items() if arm_side(k) is None or arm_side(k) == side[j]}
        t = sum(core.values())
        out.append({k: v / t for k, v in core.items()} if t > 0 else {LOWER.index("spine_03"): 1.0})
    return out


def clip_poses(binds, body):
    """The lab's clips (verse/packs/everglade.json's people) at a few moments each, as the mesh's
    palette has them (metahumanlab.coax lab_palette, with the pack's bind turns): per pose, per
    joint a 3x3 and an offset (metres) taking a bind point to the pose. Idle and walk at several
    moments, the rest once."""
    import json
    spec = json.loads((Path(__file__).resolve().parents[2] / "packs/everglade.json").read_text())["people"]
    for k, v in spec["libraries"].items():
        spec["libraries"][k] = str(people.home_path(v))
    skeletons = {k: people.Skeleton(people.load(str(people.home_path(v)))) for k, v in spec["skeletons"].items()}
    library_rest = people.Skeleton(people.load(next(iter(spec["libraries"].values())))).t
    clips = {c["name"]: c for c in people.clips(spec, skeletons, library_rest)}
    sk = skeletons[body]
    bp = [b[:3] for b in binds]; bq = [b[3:] for b in binds]
    def conj(q): return (-q[0], -q[1], -q[2], q[3])
    def rot(q, v): return people.apply(people.mat_tr((0, 0, 0), q), v)
    def qmat(q): return [list(rot(q, e)) for e in ((1, 0, 0), (0, 1, 0), (0, 0, 1))]
    moments = [("idle", f) for f in (0.0, 0.33, 0.66)] + [("walk", f) for f in (0.0, 0.25, 0.5, 0.75)] + \
              [(n, 0.5) for n in ("talk", "spell", "slash", "run", "fold")]
    out = []
    for name, f in moments:
        c = clips.get(name)
        if not c:
            continue
        t = f * c["duration"]
        def pair(keys):
            i = max(sum(1 for k in keys if k[0] <= t) - 1, 0)
            a = keys[i]; b = keys[i + 1] if i + 1 < len(keys) else a
            u = min(max((t - a[0]) / (b[0] - a[0]), 0.0), 1.0) if b[0] > a[0] else 0.0
            return a[1], b[1], u
        pel = sk.index("pelvis")
        gq, rq, gp, D = [], [], [], []
        for j in range(len(sk.names)):
            lt = sk.t[j]
            if j == pel and c["move"][body]:
                a, b, u = pair(c["move"][body]); lt = tuple(x + (y - x) * u for x, y in zip(a, b))
            lq = sk.q[j]
            if c["rot"][j]:
                a, b, u = pair(c["rot"][j]); lq = people.slerp(people.qunit(a), people.qunit(b), u)
            par = sk.parent[j]
            if par < 0:
                gq.append(lq); rq.append(sk.q[j]); gp.append(add(bp[j], sub(lt, sk.t[j])))
            else:
                gq.append(people.qunit(people.qmul(gq[par], lq))); rq.append(people.qunit(people.qmul(rq[par], sk.q[j])))
                gp.append(add(add(gp[par], rot(D[par], sub(bp[j], bp[par]))), rot(gq[par], sub(lt, sk.t[j]))))
            D.append(people.qunit(people.qmul(people.qmul(gq[j], conj(rq[j])), bq[j])))
        out.append((f"{name}@{f:.2f}", [(tr(qmat(D[j])), sub(gp[j], rot(D[j], bp[j]))) for j in range(len(sk.names))]))
    return out


def edge_check(pos, fw, tris, poses):
    """Each garment edge skinned, by its packed weights, into the lab's clips (clip_poses), against
    its authored length: per pose the share stretched past 1.5x and the worst. Torn cloth (weights
    mixing joints that part, as a hanging hand's and the hip's) shows as long edges: per pose also
    the share grown by more than 1 cm and half again and the most grown (cm)."""
    edges = set()
    for t in tris:
        for a, b in ((t[0], t[1]), (t[1], t[2]), (t[2], t[0])):
            if math.dist(pos[a], pos[b]) >= 0.05:
                edges.add((min(a, b), max(a, b)))
    res = {}
    for name, mats in poses:
        Y = []
        for x, w in zip(pos, fw):
            xm = tuple(c / 100.0 for c in x); q = (0.0, 0.0, 0.0)
            for k, a in w:
                A, c = mats[k]
                q = add(q, tuple(v * a for v in add(mv(A, xm), c)))
            Y.append(q)
        over = grown = 0; worst = 1.0; most = 0.0
        for a, b in edges:
            l0 = math.dist(pos[a], pos[b]); l1 = math.dist(Y[a], Y[b]) * 100.0
            over += l1 > 1.5 * l0; worst = max(worst, l1 / l0)
            grown += l1 - l0 > 1.0 and l1 > 1.5 * l0; most = max(most, l1 - l0)
        res[name] = (over / max(len(edges), 1), worst, grown / max(len(edges), 1), most)
    return res
