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
  - the mesh is posed from the DNA's bind (MetaHuman's A pose) into the rig's T pose: each kept
    joint turns, in world space, by the shortest arc from its bone's bind direction to the rig's
    rest direction for that bone, and its children follow (linear blend skinning with the DNA's
    own weights), so the mesh keeps its proportions;
  - positions go to metres, Y up, +Z forward, the figure's left +x (the rig's axes);
  - weights keep the four heaviest after folding, summing to 255.

The pack's bind positions are the DNA's kept joints in that T pose. Nothing this reads or writes
goes into git or is used to train AI models (the export's NO-TRAINING.txt).
"""
import math, os, subprocess
from pathlib import Path

import people

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
# shell (the cornea's wet layer), the eye edge (its occlusion), saliva and the tear ducts wait for
# the passes that blend them.
MESHES = {
    "body_lod0_mesh": ("skin", "Body_Basecolor.png", "Body_Normal.png", "Body_SRMF.png"),
    "head_lod0_mesh": ("skin", "Head_Basecolor.png", "Head_Normal.png", "Head_SRMF.png"),
    "teeth_lod0_mesh": ("teeth", "Teeth_Color.png", "Teeth_Normal.png", None),
    "eyeLeft_lod0_mesh": ("eye", "Eyes_Color.png", "Eyes_Normal.png", None),
    "eyeRight_lod0_mesh": ("eye", "Eyes_Color.png", "Eyes_Normal.png", None),
    "eyelashes_lod0_mesh": ("brow", "Eyelashes_Color.png", None, None),
}


def maps_dir(export):
    found = sorted(export.glob("dcc/*/Maps"))
    if not found:
        raise SystemExit(f"metahuman: no dcc/*/Maps under {export}")
    return found[0]


def mesh(path, skin, J, W, p, P, flip_v):
    """One DNA mesh in the rig's T pose, metres: vertices {p, n, uv, j, w} (one per distinct
    position, uv and normal a face corner uses) and triangles (its quads split)."""
    vs, vts, vns, faces = obj(path)
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
        ids = []
        for (pi, ti, ni) in f:
            key = (pi, ti, ni)
            if key not in corner:
                corner[key] = len(verts)
                ws = folded[pi]
                x = vs[pi]; nn = vns[ni]
                q = (0.0, 0.0, 0.0); m = (0.0, 0.0, 0.0); total = sum(w for _, w in ws) or 1.0
                for k, w in ws:
                    if w <= 0: continue
                    q = add(q, tuple(c * w / total for c in add(mv(W[k], sub(x, p[k])), P[k])))
                    m = add(m, tuple(c * w / total for c in mv(W[k], nn)))
                u, v = vts[ti]
                verts.append({"p": tuple(c / 100.0 for c in q), "n": unit(m), "uv": (u, 1.0 - v if flip_v else v),
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
    for name, (material, base, normal, rough) in MESHES.items():
        src = body if name.startswith("body") else head
        J = JB if src is body else JH
        path = src / f"{name}.obj"
        if not path.exists():
            continue
        verts, tris = mesh(path, skin_file(src / f"{name}.skin.txt"), J, W, p, P, flip_v)
        pack.piece(name.replace("_lod0_mesh", ""), verts, tris, material,
                   maps / base, maps / normal if normal else None, maps / rough if rough else None)
    pack.binds = [tuple(c / 100.0 for c in P[k]) for k in range(65)]
    pack.tints = {"skin": (1.0, 1.0, 1.0, 1.0), "eye": (1.0, 1.0, 1.0, 0.25), "teeth": (1.0, 1.0, 1.0, 0.5),
                  "brow": (1.0, 1.0, 1.0, 0.6), "hair": (1.0, 1.0, 1.0, 0.55), "cloth": (1.0, 1.0, 1.0, 1.0)}
    # the people rig's body nearer this one's proportions plays its clips
    def off(sk):
        return sum(math.dist(a, b) for a, b in zip(sk, pack.binds))
    female = people.Skeleton(people.load(f"{OUTFITS}/Female_Peasant.gltf"))
    pack.body = "female" if off([tuple(m[r][3] for r in range(3)) for m in female.world]) < off(rest) else "male"
    return {"flip_v": flip_v}
