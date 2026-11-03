"""Fantasy outfits for MetaHuman Lab's human packs: the Quaternius Modular Character Outfits -
Fantasy (CC0; the clothes Everglade's people and 5E Lab's classes wear), fitted onto another body.

    python3 verse/tools/humans/pack_human.py metahuman EXPORT_DIR --outfit fantasy
    python3 verse/tools/humans/pack_human.py standin --outfit fantasy

Outfits (OUTFITS): fantasy (the ranger's tunic, trousers, boots, belt and bracers), hooded (the
same with the hood, its hair clipped inside it, and a pauldron on each shoulder) and robe (the peasant's shirt,
corset and shoes under a long wool skirt, robe_skirt(), which hides the legs).

The clothes were made on the Universal Base Characters' superhero female (the outfits' own
skeleton; their arms lie 4 mm off it), in its T pose. The target is any body in the people rig's
T pose: its surface (points, normals, skin weights on the rig's 65 joints, centimetres) and its
joints. fit() moves each garment onto it in three steps, then skins it:

  1. bones: each joint's frame (along its bone, the figure's forward or up across it) from the
     source's skeleton to the target's, stretched along the bone by their lengths' ratio; a
     garment point follows its own skin's joints (linear blend);
  2. the surface: the source body, moved the same way, is projected onto the target's surface;
     each garment point adds the moves of the source body points near it (a Gaussian over a few
     centimetres), so the cloth keeps its own distance from the skin: a 4 mm sleeve stays 4 mm;
  3. the gap: a point still inside the target, or nearer it than GAP_MIN, goes out along the
     surface's normal, and so does the cloth over it (the push spreads over SPREAD_CM), so
     layers keep their order.

Skin weights come from the target's surface: per garment point, a Gaussian over the body points
near it (those the cloth is behind count a quarter), smoothed over the garment's own neighbours;
a small piece (a bracer, a belt, the hood) moves as one, its mean weights. In the T pose the arms
are clear of the trunk, so a sleeve's weights are its arm's and the side's the trunk's. The robe
hangs from the pelvis, more and more from the thighs down to the knee, split between them by
its angle round the hips. covered() finds the skin under the outfit by casting each body
point's normal against the garments' triangles.
"""
import math
from pathlib import Path

import people

OUTFITS_DIR = "~/Downloads/Modular Character Outfits - Fantasy[Standard]/Exports/glTF (Godot-Unreal)/Outfits"
BASE = "~/Downloads/Universal Base Characters[Standard]/Base Characters/Godot - UE/Superhero_Female_FullBody.gltf"
SKIN = ("MI_Regular_Female", "MI_Regular_Male")
HAND = ("hand", "index", "middle", "ring", "pinky", "thumb")      # the outfits' bare hands and necks: the target's own skin shows
RANGER = ["Female_Ranger_Body", "Female_Ranger_Arms", "Female_Ranger_Arms_Bracer", "Female_Ranger_Body_Belt_1",
          "Female_Ranger_Legs", "Female_Ranger_Feet"]
OUTFITS = {
    "fantasy": {"parts": [("Female_Ranger", RANGER)], "hair": True},
    # the hood keeps the hair, clipped to it (clipped()); the pauldron, made for the left shoulder
    # only, is mirrored onto the right and dyed leather (its grey metal reads white in the lab)
    "hooded": {"parts": [("Female_Ranger", RANGER + ["Female_Ranger_Head_Hood", "Female_Ranger_Acc_Pauldrons"])], "hair": True,
               "clip_hair": ["Female_Ranger_Head_Hood"], "mirror": ["Female_Ranger_Acc_Pauldrons"],
               "tint": {"Female_Ranger_Acc_Pauldrons": (0.24, 0.15, 0.09)}},
    # the peasant's arms end in its own hands (the peasant's image has their skin): cut at the wrist
    # and the robe hides the legs (its own trousers too): no cloth can follow a stride, so a leg
    # would poke through its front; the shoes show under its hem
    "robe": {"parts": [("Female_Peasant", ["Female_Peasant_Arms", "Female_Peasant_Body", "Female_Peasant_Feet"])],
             "hair": True, "no_hands": True, "no_legs": True,
             # the peasant's boots reach the knee: under the skirt their shafts would poke out of
             # its back in a stance, so they end this far (cm) above its hem
             "cut": {"Female_Peasant_Feet": 6.0},
             # a long skirt (robe_skirt): from just under the bodice's lower edge to a hand above
             # the ankle, close round the hips and an A-line below them
             "robe": {"top": 102.0, "hem": 6.0, "slope": 0.09}},
}
LICENCE = "Quaternius Modular Character Outfits - Fantasy|CC0 1.0"
GAP_MIN = 0.3        # cm: the least cloth stands off the skin
SNUG_CM = 0.5        # cm: the most a bodice, tunic or skirt's top stands off the skin from the hips up
SNUG_PULL = 0.95     # of the rest it is drawn in (snug()), so the cloth takes the body's waist and bust
SNUG_LAYER = 0.3     # of a layer's height over the piece's innermost cloth near it that it keeps
SNUG_NEAR = 3.0      # cm: how near that innermost cloth is looked for
SNUG_SKIP = ("Hood", "Pauldron", "Feet", "Bracer", "Arms")   # pieces kept as cut: loose by design
SPREAD_CM = 3.0      # how far a push spreads over the cloth round it
SMALL_CM = 40.0      # a piece no bigger than this every way moves as one
ROBE_SEGMENTS, ROBE_RINGS = 96, 30
ROBE_MARGIN = 1.5    # cm off the body round the waist and hips (the cinch below draws its top edge in)
ROBE_FOLDS = 15      # folds round the skirt, deepest at the hem
ROBE_FOLD_CM = 1.3   # how far a fold stands in or out at the hem
ROBE_CINCH_CM, ROBE_WAIST_CM = 14.0, 2.2   # the robe's top 14 cm drawn in to 2.2 cm off the skin at its top edge
ROBE_WOOL = (92, 24, 30)       # the skirt's wool (sRGB), a wine red under the peasant's brown bodice
ROBE_TRIM = (176, 132, 58)     # its hem's braid, an ochre
ROBE_THIGH = 0.6     # the most of a robe point's weight on the thighs (the rest the pelvis's): from the knee down


def sub(a, b): return tuple(x - y for x, y in zip(a, b))
def add(a, b): return tuple(x + y for x, y in zip(a, b))
def scale(a, k): return tuple(x * k for x in a)
def dot(a, b): return sum(x * y for x, y in zip(a, b))
def cross(a, b): return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
def unit(a):
    l = math.sqrt(dot(a, a)); return tuple(x / l for x in a) if l > 1e-12 else (0.0, 0.0, 1.0)


class Near:
    """Points (cm) in cells, for radius searches."""
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
        out.sort()
        return out
    def nearest(self, x, r=2.0, most=64.0):
        while r <= most:
            f = self.within(x, r)
            if f:
                return f
            r *= 2
        return []


def bone_ends(parent, lower):
    """Per joint the joint its bone points to (metahuman.bone_ends: a limb's along its chain)."""
    child = {}
    for j, p in enumerate(parent):
        if p >= 0 and p not in child:
            child[p] = j
    for sd in ("_l", "_r"):
        child[lower.index("hand" + sd)] = lower.index("middle_01" + sd)
        child[lower.index("clavicle" + sd)] = lower.index("upperarm" + sd)
        child[lower.index("foot" + sd)] = lower.index("ball" + sd)
    child[lower.index("spine_03")] = lower.index("neck_01")
    child[lower.index("pelvis")] = lower.index("spine_01")
    return child


def frames(src, dst, parent, lower):
    """Per joint the map from the source skeleton's frame to the target's: (origin from, 3x3, origin
    to), a point x going to to + A (x - from)."""
    ends = bone_ends(parent, lower)
    out = [None] * len(src)
    for k in range(len(src)):
        c = ends.get(k)
        if c is None or math.dist(src[c], src[k]) < 1e-6:
            A = out[parent[k]][1] if parent[k] >= 0 else [[1, 0, 0], [0, 1, 0], [0, 0, 1]]
            out[k] = (src[k], A, dst[k]); continue
        a_s, a_t = unit(sub(src[c], src[k])), unit(sub(dst[c], dst[k]))
        ref = (0.0, 0.0, 1.0) if abs(a_s[2]) < 0.8 else (0.0, 1.0, 0.0)
        def frame(a):
            b = unit(sub(ref, scale(a, dot(a, ref)))); return a, b, cross(a, b)
        Fs, Ft = frame(a_s), frame(a_t)
        s = min(2.0, max(0.5, math.dist(dst[c], dst[k]) / math.dist(src[c], src[k])))
        # A = Ft diag(s, 1, 1) Fs^T (frames as columns)
        A = [[sum(Ft[m][i] * (s if m == 0 else 1.0) * Fs[m][j] for m in range(3)) for j in range(3)] for i in range(3)]
        out[k] = (src[k], A, dst[k])
    return out


def mv(A, v): return tuple(sum(A[i][k] * v[k] for k in range(3)) for i in range(3))


def follow(x, n, ws, F):
    """A point and normal moved by its joints' frame maps (linear blend)."""
    p = (0.0, 0.0, 0.0); q = (0.0, 0.0, 0.0)
    for k, w in ws:
        o, A, t = F[k]
        p = add(p, scale(add(t, mv(A, sub(x, o))), w)); q = add(q, scale(mv(A, n), w))
    return p, unit(q)


def source(outfit, sex="female"):
    """The outfit's pieces in the source's T pose (cm): per piece its name, material (glTF's), images
    (base, normal, rough), vertices {x, n, uv, ws (rig joint, weight)} and triangles; and the
    source body's surface (points, normals, weights) and joints."""
    spec = OUTFITS[outfit]
    sk = people.Skeleton(people.load(f"{OUTFITS_DIR}/Female_Ranger.gltf"))
    lower = [n.lower() for n in sk.names]
    skel = {"female": sk}
    def verts(pr):
        return [{"x": scale(v["p"], 100.0), "n": v["n"], "uv": v["uv"],
                 "ws": [(j, w / 255.0) for j, w in zip(v["j"], v["w"]) if w > 0]} for v in pr["verts"]]
    joints = [tuple(m[r][3] * 100.0 for r in range(3)) for m in sk.world]
    pieces = []
    for stem, meshes in spec["parts"]:
        path = f"{OUTFITS_DIR}/{stem}.gltf"
        g = people.load(path)
        names = meshes or [g["meshes"][n["mesh"]]["name"] for n in g["nodes"] if "mesh" in n]
        mats = {m["name"]: m for m in g["materials"]}
        def img(t):
            return g["dir"] / g["images"][g["textures"][t["index"]]["source"]]["uri"] if t else None
        for name in names:
            for pr in people.part_file({"name": name, "body": "female"}, path, [name], skel):
                if pr["material"] in SKIN:
                    continue
                if spec.get("no_hands"):
                    hand = {lower.index(n) for n in lower if n.split("_")[0] in HAND}
                    share = [sum(w for j, w in zip(v["j"], v["w"]) if j in hand) / 255.0 for v in pr["verts"]]
                    tris = [t for t in pr["tris"] if max(share[i] for i in t) < 0.5]
                    if len(tris) < len(pr["tris"]):
                        used = sorted({i for t in tris for i in t}); renum = {i: k for k, i in enumerate(used)}
                        pr = {**pr, "verts": [pr["verts"][i] for i in used], "tris": [tuple(renum[i] for i in t) for t in tris]}
                if name in spec.get("cut", {}):
                    top = (joints[lower.index("foot_l")][1] + spec["robe"]["hem"] + spec["cut"][name]) / 100.0
                    tris = [t for t in pr["tris"] if max(pr["verts"][i]["p"][1] for i in t) <= top]
                    used = sorted({i for t in tris for i in t}); renum = {i: k for k, i in enumerate(used)}
                    pr = {**pr, "verts": [pr["verts"][i] for i in used], "tris": [tuple(renum[i] for i in t) for t in tris]}
                m = mats[pr["material"]]; pbr = m.get("pbrMetallicRoughness", {})
                pc = {"name": name, "material": pr["material"],
                      "rgb": spec.get("tint", {}).get(name, tuple(pbr.get("baseColorFactor", (1, 1, 1, 1))[:3])),
                      "rough": pbr.get("roughnessFactor", 1.0),
                      "images": (img(pbr.get("baseColorTexture")), img(m.get("normalTexture")), img(pbr.get("metallicRoughnessTexture"))),
                      "verts": verts(pr), "tris": pr["tris"], "own_weights": False}
                if not any(k in name for k in SNUG_SKIP):
                    while len(pc["tris"]) < SUBDIVIDE_TRIS:
                        pc = subdivided(pc)
                pieces.append(pc)
                if name in spec.get("mirror", ()):
                    pieces.append(mirrored(pc, lower))
    body = [v for pr in people.part_file({"name": "base", "body": "female"}, BASE, None, skel)
            if pr["material"].startswith("MI_Superhero") for v in verts(pr)]
    if "robe" in spec:
        verts_, tris = robe_skirt(spec["robe"], [b["x"] for b in body], joints, lower)
        base, normal = robe_cloth()
        pieces.append({"name": "Robe", "material": "MI_Peasant", "rgb": (1.0, 1.0, 1.0), "rough": 0.92,
                       "images": (base, normal, None), "verts": verts_, "tris": tris, "own_weights": True})
    return pieces, body, joints, sk.parent, lower


SUBDIVIDE_TRIS = 12000   # a trunk piece is split until it has at least this many triangles


def subdivided(pc):
    """A piece with each triangle split in four at its edges' middles (positions, normals, uv and
    weights averaged; an edge's middle shared by the triangles on it, so the surface stays closed).
    The outfits' bodices are a couple of thousand triangles: drawn onto a woman's bust and waist
    (snug()) so few points make a faceted cone; split, they take the body's curve."""
    vs = list(pc["verts"]); mid = {}
    def middle(a, b):
        k = (a, b) if a < b else (b, a)
        if k not in mid:
            va, vb = vs[a], vs[b]
            ws = {}
            for j, w in list(va["ws"]) + list(vb["ws"]):
                ws[j] = ws.get(j, 0.0) + 0.5 * w
            top = sorted(ws.items(), key=lambda kv: -kv[1])[:4]
            t = sum(w for _, w in top) or 1.0
            vs.append({"x": scale(add(va["x"], vb["x"]), 0.5), "n": unit(add(va["n"], vb["n"])),
                       "uv": tuple(0.5 * (p + q) for p, q in zip(va["uv"], vb["uv"])),
                       "ws": [(j, w / t) for j, w in top]})
            mid[k] = len(vs) - 1
        return mid[k]
    tris = []
    for a, b, c in pc["tris"]:
        ab, bc, ca = middle(a, b), middle(b, c), middle(c, a)
        tris += [(a, ab, ca), (ab, b, bc), (ca, bc, c), (ab, bc, ca)]
    return {**pc, "verts": vs, "tris": tris}


def creased_normals(pc):
    """Each vertex's normal from the triangles round its position that face its own way (within
    60 degrees): smooth across the piece, a crease or a cloth's two sides kept apart."""
    vs = pc["verts"]
    fn = []
    by_pos = {}
    for t in pc["tris"]:
        a, b, c = (vs[i]["x"] for i in t)
        n = cross(sub(b, a), sub(c, a))
        fn.append(n)
        for i in t:
            by_pos.setdefault(tuple(round(q * 20) for q in vs[i]["x"]), []).append(len(fn) - 1)
    for v in vs:
        acc = (0.0, 0.0, 0.0)
        for f in by_pos.get(tuple(round(q * 20) for q in v["x"]), ()):
            if dot(unit(fn[f]), v["n"]) > 0.5:
                acc = add(acc, fn[f])
        if dot(acc, acc) > 1e-12:
            v["n"] = unit(acc)


def mirrored(pc, lower):
    """A piece mirrored across the figure's middle (x to -x): its triangles turned to keep facing
    out, its weights moved to the other side's joints."""
    other = {k: lower.index(n[:-2] + ("_r" if n.endswith("_l") else "_l")) if n.endswith(("_l", "_r")) else k
             for k, n in enumerate(lower)}
    vs = [{**v, "x": (-v["x"][0], v["x"][1], v["x"][2]), "n": (-v["n"][0], v["n"][1], v["n"][2]),
           "ws": [(other[j], w) for j, w in v["ws"]]} for v in pc["verts"]]
    return {**pc, "name": pc["name"] + "_Mirrored", "verts": vs, "tris": [(t[0], t[2], t[1]) for t in pc["tris"]]}


def welded(verts, tris):
    """Per vertex its weld (positions within half a millimetre are one) and the welds' neighbours."""
    key_of, keys = {}, []
    for v in verts:
        q = tuple(round(c * 20) for c in v["x"])
        if q not in key_of:
            key_of[q] = len(key_of)
        keys.append(key_of[q])
    nbr = [set() for _ in range(len(key_of))]
    for a, b, c in tris:
        a, b, c = keys[a], keys[b], keys[c]
        nbr[a].update((b, c)); nbr[b].update((a, c)); nbr[c].update((a, b))
    return keys, nbr


def components(nbr):
    seen = [-1] * len(nbr); n = 0
    for s in range(len(nbr)):
        if seen[s] >= 0:
            continue
        stack = [s]; seen[s] = n
        while stack:
            a = stack.pop()
            for b in nbr[a]:
                if seen[b] < 0:
                    seen[b] = n; stack.append(b)
        n += 1
    return seen, n


def fit(outfit, target, log=print):
    """The outfit on the target body. target: {"x", "n", "w"} per surface point (cm, unit normals,
    {rig joint: weight}) and "joints" (65 positions, cm), all in the people rig's T pose. Returns the
    pieces with each vertex's "x", "n" and "ws" (its weights) in the target's T pose."""
    pieces, body, src_joints, parent, lower = source(outfit)
    F = frames(src_joints, target["joints"], parent, lower)
    # 1. bones
    for pc in pieces:
        for v in pc["verts"]:
            v["x"], v["n"] = follow(v["x"], v["n"], v["ws"], F)
    sb = [follow(v["x"], v["n"], v["ws"], F) for v in body]
    # 2. the surface: each moved source body point projected onto the target's surface
    tx, tn = target["x"], target["n"]
    tnear = Near(tx)
    moves = []
    for b, bn in sb:
        found = [(d, i) for d, i in tnear.nearest(b, 2.0, 32.0)[:24] if dot(tn[i], bn) > 0.3][:4]
        if not found:
            moves.append((0.0, 0.0, 0.0)); continue
        acc, tot = (0.0, 0.0, 0.0), 0.0
        for d, i in found:
            f = 1.0 / (d + 0.2)
            m = sub(b, scale(tn[i], dot(sub(b, tx[i]), tn[i])))      # b on point i's tangent plane
            acc = add(acc, scale(sub(m, b), f)); tot += f
        moves.append(scale(acc, 1.0 / tot))
    bnear = Near([b for b, _ in sb])
    shifts = []
    for pc in pieces:
        for v in pc["verts"]:
            found = bnear.nearest(v["x"], 2.0, 64.0)
            if not found:
                continue
            sigma = max(2.0, found[0][0])
            acc, tot = (0.0, 0.0, 0.0), 0.0
            for d, i in bnear.within(v["x"], found[0][0] + 2.0 * sigma):
                f = math.exp(-(d / sigma) ** 2)
                acc = add(acc, scale(moves[i], f)); tot += f
            mvd = scale(acc, 1.0 / tot)
            shifts.append(math.sqrt(dot(mvd, mvd)))
            v["x"] = add(v["x"], mvd)
    ss = sorted(shifts)
    log(f"outfit {outfit}: the surface step moved the cloth 50/95/max% " +
        " ".join(f"{ss[int((len(ss) - 1) * q)]:.1f}" for q in (0.5, 0.95, 1.0)) + " cm")
    # the robe's waist drawn in to the body (its rings are cut wide enough to clear a stride)
    for pc in pieces:
        if not pc["own_weights"]:
            continue
        top = max(v["x"][1] for v in pc["verts"])
        for v in pc["verts"]:
            t = min(1.0, max(0.0, (v["x"][1] - (top - ROBE_CINCH_CM)) / ROBE_CINCH_CM))
            found = tnear.nearest(v["x"], 4.0, 32.0) if t > 0 else []
            if found:
                i = found[0][1]; sd = dot(sub(v["x"], tx[i]), tn[i])
                if sd > ROBE_WAIST_CM:
                    v["x"] = add(v["x"], scale(tn[i], t * t * (ROBE_WAIST_CM - sd)))
    snug(pieces, target, tnear, lower, log)
    # 3. the gap: pushes off the skin, spread over the cloth round them (all pieces, so layers keep order)
    allv = [v for pc in pieces for v in pc["verts"]]
    push = []
    for v in allv:
        found = tnear.nearest(v["x"], 2.0, 8.0)
        p = (0.0, 0.0, 0.0)
        if found:
            d, i = found[0]
            s = dot(sub(v["x"], tx[i]), tn[i])
            if s < GAP_MIN and d < 4.0:
                p = scale(tn[i], GAP_MIN - s)
        push.append(p)
    pushed = [i for i, p in enumerate(push) if dot(p, p) > 0]
    if pushed:
        cnear = Near([allv[i]["x"] for i in pushed])
        spread = []
        for v in allv:
            best = (0.0, 0.0, 0.0); bl = 0.0
            for d, j in cnear.within(v["x"], SPREAD_CM):
                p = scale(push[pushed[j]], 1.0 - d / SPREAD_CM)
                l = dot(p, p)
                if l > bl:
                    best, bl = p, l
            spread.append(best)
        for v, p in zip(allv, spread):
            v["x"] = add(v["x"], p)
    log(f"outfit {outfit}: {len(pushed)} of {len(allv)} cloth points were under {GAP_MIN} cm off the skin (pushed out)")
    # 4. weights from the target's surface; the robe keeps its own, hung from the pelvis
    tw = target["w"]
    pelvis = lower.index("pelvis")
    for pc in pieces:
        if pc["own_weights"]:
            for v in pc["verts"]:
                v["ws"] = robe_weights(v["x"], target["joints"], lower)
            continue
        keys, nbr = welded(pc["verts"], pc["tris"])
        kw = [None] * len(nbr)
        for v, k in zip(pc["verts"], keys):
            if kw[k] is not None:
                continue
            found = tnear.nearest(v["x"], 2.0, 64.0)
            sigma = max(1.0, 1.5 * found[0][0])
            acc = {}
            for d, i in found[:24]:
                f = math.exp(-(d / sigma) ** 2)
                if dot(sub(v["x"], tx[i]), tn[i]) < -0.3:
                    f *= 0.25
                for j, w in tw[i].items():
                    acc[j] = acc.get(j, 0.0) + f * w
            t = sum(acc.values()) or 1.0
            kw[k] = {j: w / t for j, w in acc.items()}
        for _ in range(6):
            nxt = []
            for i, w in enumerate(kw):
                if not nbr[i]:
                    nxt.append(w); continue
                acc = {j: 0.5 * x for j, x in w.items()}
                f = 0.5 / len(nbr[i])
                for o in nbr[i]:
                    for j, x in kw[o].items():
                        acc[j] = acc.get(j, 0.0) + f * x
                nxt.append(acc)
            kw = nxt
        # a small piece (a bracer, a belt, the hood) moves as one
        comp, n = components(nbr)
        pts = {}
        for v, k in zip(pc["verts"], keys):
            pts.setdefault(comp[k], []).append(v["x"])
        for c in range(n):
            xs = pts.get(c, [])
            if not xs:
                continue
            ext = max(max(x[i] for x in xs) - min(x[i] for x in xs) for i in range(3))
            if ext < SMALL_CM:
                members = [k for k in range(len(kw)) if comp[k] == c]
                acc = {}
                for k in members:
                    for j, x in kw[k].items():
                        acc[j] = acc.get(j, 0.0) + x / len(members)
                for k in members:
                    kw[k] = dict(acc)
        for v, k in zip(pc["verts"], keys):
            top = sorted(kw[k].items(), key=lambda kv: -kv[1])[:4]
            t = sum(w for _, w in top) or 1.0
            v["ws"] = [(j, w / t) for j, w in top]
    for pc in pieces:
        if pc["own_weights"]:
            renormal(pc)
    return pieces


def snug(pieces, target, tnear, lower, log=print):
    """The cloth over the trunk drawn onto the body: a point of a bodice, tunic, belt or the
    skirt's top (not SNUG_SKIP's) more than SNUG_CM off the target's skin goes SNUG_PULL of the way
    in along the skin's normal, from the shoulders down to the hips, fading out over 12 cm below
    them (a tunic's skirt keeps its flare); the moves are smoothed over each piece's neighbours.
    The outfits were cut for the superhero female, whose trunk is straighter than a woman's: this
    gives the clothes her waist and bust instead of a tube."""
    J = target["joints"]; tx, tn = target["x"], target["n"]
    hip = J[lower.index("thigh_l")][1]
    moved = []
    for pc in pieces:
        if any(k in pc["name"] for k in SNUG_SKIP):
            continue
        keys, nbr = welded(pc["verts"], pc["tris"])
        # per weld: how far it stands off the skin facing it, along that skin's normal
        off = [None] * len(nbr); at = [None] * len(nbr); fade = [0.0] * len(nbr)
        for v, k in zip(pc["verts"], keys):
            if at[k] is not None:
                continue
            at[k] = v["x"]
            y = v["x"][1]
            fade[k] = f = 1.0 if y >= hip else max(0.0, 1.0 - (hip - y) / 12.0)
            # skin facing the same way as the cloth (not the bust's underside the cloth bridges)
            found = []
            for r in ((2.0, 4.0, 8.0, 16.0) if f > 0 else ()):
                found = [(d, i) for d, i in tnear.within(v["x"], r) if dot(tn[i], v["n"]) > 0.6][:4]
                if found:
                    break
            if found:
                n = unit(tuple(sum(tn[i][q] for _, i in found) for q in range(3)))
                off[k] = (n, sum(dot(sub(v["x"], tx[i]), n) for _, i in found) / len(found))
        # a piece's own layers (a bodice over a shirt) keep their order: the innermost cloth near
        # a point goes to SNUG_CM, and a layer over it keeps SNUG_LAYER of its height above that,
        # so two layers never meet (where they did, the shirt showed through the bodice in spots)
        wnear = Near([x if o else (1e6, 1e6, 1e6) for x, o in zip(at, off)])
        mv_ = []
        for k in range(len(nbr)):
            m = (0.0, 0.0, 0.0)
            if off[k] is not None:
                n, sd = off[k]
                floor = min([off[i][1] for d, i in wnear.within(at[k], SNUG_NEAR) if off[i] is not None] or [sd])
                want = SNUG_CM + max(0.0, sd - floor) * SNUG_LAYER
                if sd > want:
                    m = scale(n, -fade[k] * SNUG_PULL * (sd - want))
            mv_.append(m)
        # smoothed over about the same span of cloth however finely it was split
        for _ in range(4 * max(1, round(math.sqrt(len(pc["tris"]) / 2500.0)))):
            mv_ = [scale(add(scale(m, 1.0), scale(tuple(sum(mv_[o][q] for o in nbr[i]) for q in range(3)), 1.0 / len(nbr[i]))), 0.5)
                   if nbr[i] else m for i, m in enumerate(mv_)]
        for v, k in zip(pc["verts"], keys):
            v["x"] = add(v["x"], mv_[k])
        creased_normals(pc)
        moved += [math.sqrt(dot(m, m)) for m in mv_]
    if moved:
        ms = sorted(moved)
        log(f"snug: the trunk's cloth drawn in 50/95/max% " + " ".join(f"{ms[int((len(ms) - 1) * q)]:.1f}" for q in (0.5, 0.95, 1.0)) + " cm")


def robe_weights(x, J, lower):
    """A skirt point's weights: the pelvis's at the waist, more and more the thighs' down to the
    knee (ROBE_THIGH of it), split between them smoothly by its angle round the hips (so the
    stretch between the legs spreads round the skirt instead of tearing its front)."""
    pelvis, tl, tr_ = lower.index("pelvis"), lower.index("thigh_l"), lower.index("thigh_r")
    hip = J[tl][1]; knee = J[lower.index("calf_l")][1]
    mid = (J[tl][0] + J[tr_][0]) / 2; zc = J[pelvis][2]
    dx, y, dz = x[0] - mid, x[1], x[2] - zc
    a = ROBE_THIGH * min(1.0, max(0.0, (hip + 4.0 - y) / (hip + 4.0 - knee)))
    # the thigh_l (+x) side's share falls evenly round the skirt (by arc, not by x: the stretch
    # between the legs as they part spreads over the whole front and back alike)
    off = abs((math.atan2(dx, dz) - math.pi / 2 + math.pi) % (2 * math.pi) - math.pi)   # from the +x side
    sd = 1.0 - off / math.pi
    ws = [(pelvis, 1.0 - a), (tl, a * sd), (tr_, a * (1.0 - sd))]
    return sorted([(j, w) for j, w in ws if w > 1e-4], key=lambda kv: -kv[1])


def robe_skirt(rb, body, J, lower):
    """A long skirt round the source body (cm, its T pose): ROBE_RINGS rings from rb["top"] down
    to rb["hem"] above the ankle. Down to the widest ring (the hips) each ring is the body's
    extent at its height plus ROBE_MARGIN; below, an A-line, rb["slope"] out per centimetre down.
    ROBE_FOLDS soft folds stand out round it, none at the waist and ROBE_FOLD_CM deep at the hem,
    whose edge rises and falls with them. Outside and inside faces; uv round it and down it."""
    top, hem = rb["top"], J[lower.index("foot_l")][1] + rb["hem"]
    R, S = ROBE_RINGS, ROBE_SEGMENTS
    ys = [top + (hem - top) * (i / (R - 1)) for i in range(R)]
    knee = J[lower.index("calf_l")][1]
    ext = []
    for y in ys:
        band = [b for b in body if abs(b[1] - y) < 2.0 and abs(b[0]) < 35.0]
        if not band and y > knee + 10.0:
            raise SystemExit(f"outfits: no body at {y:.0f} cm for the skirt")
        z0, z1 = (min(b[2] for b in band), max(b[2] for b in band)) if band else (0.0, 0.0)
        ext.append((max(abs(b[0]) for b in band) if band else 0.0, (z1 - z0) / 2, (z0 + z1) / 2))
    w = max((i for i in range(R) if ys[i] > knee + 10.0), key=lambda i: ext[i][0])
    rings, rx, rz, cz = [], 0.0, 0.0, 0.0
    for i, y in enumerate(ys):
        if i <= w:
            rx = max(rx, ext[i][0] + ROBE_MARGIN); rz = max(rz, ext[i][1] + ROBE_MARGIN); cz = ext[i][2]
            rings.append((y, rx, rz, cz))
        else:
            d = ys[w] - y
            rings.append((y, rx + rb["slope"] * d, rz + rb["slope"] * 1.2 * d, cz))
    # the folds: two harmonics with fixed phases, so each fold differs a little from the next
    def fold(a):
        return 0.75 * math.cos(ROBE_FOLDS * a + 0.6) + 0.25 * math.cos((2 * ROBE_FOLDS - 3) * a + 2.1)
    def depth(y):
        t = min(1.0, max(0.0, (ys[w] - y) / max(ys[w] - hem, 1.0)))
        return ROBE_FOLD_CM * t * t * (3 - 2 * t)
    verts, tris = [], []
    for face in (1, -1):
        base = len(verts)
        for i, (y, rx, rz, cz) in enumerate(rings):
            for k in range(S + 1):                      # the seam's column twice, for its uv
                a = 2 * math.pi * k / S
                f = fold(a); dr = depth(y) * f
                yy = y + (0.8 * f if i == R - 1 else 0.0)
                x = (rx + dr) * math.sin(a); z = cz + (rz + dr) * math.cos(a)
                p = (x, yy, z)
                verts.append({"x": p, "n": (0.0, 0.0, 1.0), "uv": (k / S, i / (R - 1)),
                              "ws": robe_weights(p, J, lower)})
        for i in range(R - 1):
            for k in range(S):
                a = base + i * (S + 1) + k; b = a + 1; c = a + S + 1; d = c + 1
                # seen from outside, (a, c, b) runs counter-clockwise: a round the skirt, c below it
                tris += [(a, c, b), (b, c, d)] if face > 0 else [(a, b, c), (b, d, c)]
    pc = {"verts": verts, "tris": tris}
    renormal(pc)
    # outside faces out: their normals point away from the axis
    v0 = verts[(S + 1) * (R // 2)]
    if v0["n"][2] * (v0["x"][2] - rings[R // 2][3]) + v0["n"][0] * v0["x"][0] < 0:
        tris = [(t[0], t[2], t[1]) for t in tris]
        pc["tris"] = tris
        renormal(pc)
    return verts, tris


def renormal(pc):
    """Each vertex's normal from its triangles (area weighted), the seam's twins shared."""
    acc = {}
    vs = pc["verts"]
    key = [(round(v["x"][0], 3), round(v["x"][1], 3), round(v["x"][2], 3), i >= len(vs) // 2) for i, v in enumerate(vs)]
    for t in pc["tris"]:
        a, b, c = (vs[i]["x"] for i in t)
        n = cross(sub(b, a), sub(c, a))
        for i in t:
            k = key[i]; o = acc.get(k, (0.0, 0.0, 0.0)); acc[k] = add(o, n)
    for v, k in zip(vs, key):
        v["n"] = unit(acc.get(k, (0.0, 0.0, 1.0)))


def robe_cloth(size=2048):
    """The skirt's wool (generated, CC0): a twill weave over a slow mottle, with an ochre braid
    along the hem; and its normal map (green up) from the weave's height. Cached under
    ~/.openagents/verse/build/cloth."""
    from PIL import Image, ImageFilter
    out = Path.home() / ".openagents/verse/build/cloth"
    base, normal = out / f"robe-wool-{size}.png", out / f"robe-wool-{size}-normal.png"
    if base.exists() and normal.exists():
        return base, normal
    out.mkdir(parents=True, exist_ok=True)
    TW, TH = 16, 16                          # a tile: 4 threads each way, a 2/2 twill
    hgt = [[0.0] * TW for _ in range(TH)]
    for y in range(TH):
        for x in range(TW):
            tx, ty = x // 4, y // 4
            warp = (tx - ty) % 4 < 2         # the warp thread is on top here
            fx, fy = (x % 4 + 0.5) / 4, (y % 4 + 0.5) / 4
            # a thread's round profile across it, over the one under it
            hgt[y][x] = (0.6 + 0.4 * math.sin(math.pi * fx)) if warp else (0.6 + 0.4 * math.sin(math.pi * fy))
            if not warp:
                hgt[y][x] -= 0.25
    tile_n = Image.new("RGB", (TW, TH)); tile_b = Image.new("L", (TW, TH))
    for y in range(TH):
        for x in range(TW):
            gx = (hgt[y][(x + 1) % TW] - hgt[y][(x - 1) % TW]) * 0.5
            gy = (hgt[(y + 1) % TH][x] - hgt[(y - 1) % TH][x]) * 0.5
            k = 1.6
            n = unit((-gx * k, gy * k, 1.0))     # green up: +y up the image
            tile_n.putpixel((x, y), tuple(int(round(127.5 + 127.5 * c)) for c in n))
            tile_b.putpixel((x, y), int(round(200 + 55 * hgt[y][x])))
    nm = Image.new("RGB", (size, size)); shade = Image.new("L", (size, size))
    for y in range(0, size, TH):
        for x in range(0, size, TW):
            nm.paste(tile_n, (x, y)); shade.paste(tile_b, (x, y))
    mottle = Image.effect_noise((size // 32, size // 32), 40).resize((size, size), Image.BICUBIC).filter(ImageFilter.GaussianBlur(6))
    grain = Image.effect_noise((size, size), 18).filter(ImageFilter.GaussianBlur(0.6))
    wool = Image.new("RGB", (size, size), ROBE_WOOL)
    px, sh, mo, gr = wool.load(), shade.load(), mottle.load(), grain.load()
    trim0, trim1 = int(size * 0.955), int(size * 0.985)    # the braid, near the hem (v runs down)
    for y in range(size):
        braid = trim0 <= y < trim1
        for x in range(size):
            f = sh[x, y] / 255.0 * (1.0 + (mo[x, y] - 128) / 900.0 + (gr[x, y] - 128) / 1400.0)
            if braid:
                # a diamond chain on the braid
                u = (x % 64) / 64.0; v = (y - trim0) / (trim1 - trim0)
                on = abs(u - 0.5) + abs(v - 0.5) < 0.42
                c = ROBE_TRIM if on else tuple(int(q * 0.55) for q in ROBE_TRIM)
            else:
                c = ROBE_WOOL
            px[x, y] = tuple(min(255, int(q * f)) for q in c)
    wool.save(base); nm.save(normal)
    return base, normal


def covered(body_x, body_n, body_w, pieces, out_cm=3.0, in_cm=1.0):
    """The body points under the cloth (cm, the pieces' "x" in the same pose): the line along a
    skin point's normal meets a garment's triangle from in_cm inside the skin (a bulkier body than
    the outfit's pokes through there, and the cloth is what should show) to out_cm outside, and
    that garment moves with the point (a third or more of their weights shared), so a hand by the
    hip still shows and the neck over a collar's edge isn't cut."""
    cell = 2.0
    grid, tris = {}, []
    for pc in pieces:
        vs = pc["verts"]
        for t in pc["tris"]:
            a, b, c = (vs[i]["x"] for i in t)
            k = len(tris); tris.append((a, b, c, dict(vs[t[0]]["ws"])))
            lo = [int(math.floor(min(a[q], b[q], c[q]) / cell)) for q in range(3)]
            hi = [int(math.floor(max(a[q], b[q], c[q]) / cell)) for q in range(3)]
            for x in range(lo[0], hi[0] + 1):
                for y in range(lo[1], hi[1] + 1):
                    for z in range(lo[2], hi[2] + 1):
                        grid.setdefault((x, y, z), []).append(k)
    out = set()
    for i, (x, n) in enumerate(zip(body_x, body_n)):
        seen = set()
        steps = int((out_cm + in_cm) / (cell * 0.5)) + 1
        for s_ in range(steps + 1):
            p = add(x, scale(n, -in_cm + (out_cm + in_cm) * s_ / steps))
            key = tuple(int(math.floor(c / cell)) for c in p)
            for k in grid.get(key, ()):
                if k in seen:
                    continue
                seen.add(k)
                a, b, c, w = tris[k]
                t = ray_tri(x, n, a, b, c)
                if t is not None and -in_cm <= t <= out_cm and \
                        sum(min(v, w.get(j, 0.0)) for j, v in body_w[i].items()) >= 0.3:
                    out.add(i); break
            if i in out:
                break
    return out


PEEK_CELL = 0.5      # cm: the peek check's raster
PEEK_REACH = 8.0     # cm: how far round a skin cell the check looks for cloth
PEEK_MAX = 1.0       # cm2: the most skin a pose and view may show through the cloth (idle, walk)
OPEN = ("head", "neck", "hand", "index", "middle", "ring", "pinky", "thumb")   # skin meant to show


def peeks(body_x, body_w, body_tris, pieces, poses, lower, log=print, under_tris=()):
    """Skin seen through the clothes, without drawing a frame: in each pose (idle and walk; poses as
    clip_poses) and from the front, back and both sides, the body's kept triangles and the
    garments are rasterized (PEEK_CELL, orthographic) into a depth buffer; a cell whose nearest
    surface is skin meant to be covered (not the head's, neck's or hands': OPEN) and that has cloth
    on all four sides within PEEK_REACH is skin peeking through a hole or poking out of the cloth.
    Positions in cm (the pieces' "x" and body_x in the same bind); returns, per pose and view, the
    area (cm2) and the joints it lies on most."""
    open_j = {k for k, n in enumerate(lower) if n.split("_")[0] in OPEN}
    meant = [sum(w for k, w in (ws.items() if isinstance(ws, dict) else ws) if k in open_j) < 0.2 for ws in body_w]
    def skinned(x, ws, mats):
        xm = tuple(c / 100.0 for c in x); q = (0.0, 0.0, 0.0)
        for k, a in (ws.items() if isinstance(ws, dict) else ws):
            A, c = mats[k]
            q = add(q, tuple(v * a for v in add(mv(A, xm), c)))
        return scale(q, 100.0)
    views = {"front": (0, 1, 2, 1), "back": (0, 1, 2, -1), "left": (2, 1, 0, 1), "right": (2, 1, 0, -1)}
    out = {}
    for name, mats in poses:
        if not name.startswith(("idle@0.33", "walk@0.25", "walk@0.75")):
            continue
        bx = [skinned(x, w, mats) for x, w in zip(body_x, body_w)]
        cloth = []
        for pc in pieces:
            px = [skinned(v["x"], v["ws"], mats) for v in pc["verts"]]
            cloth += [tuple(px[i] for i in t) for t in pc["tris"]]
        tris = [(tuple(bx[i] for i in t), 1 if all(meant[i] for i in t) else 2, t[0]) for t in body_tris] + \
               [(t, 0, -1) for t in cloth] + [(tuple(bx[i] for i in t), 0, -1) for t in under_tris]
        for view, (ax, ay, az, sg) in views.items():
            buf = {}
            for (a, b, c), kind, src in tris:
                pa, pb, pc_ = ((p[ax] / PEEK_CELL, p[ay] / PEEK_CELL, sg * p[az]) for p in (a, b, c))
                x0, x1 = int(math.floor(min(pa[0], pb[0], pc_[0]))), int(math.ceil(max(pa[0], pb[0], pc_[0])))
                y0, y1 = int(math.floor(min(pa[1], pb[1], pc_[1]))), int(math.ceil(max(pa[1], pb[1], pc_[1])))
                den = (pb[1] - pc_[1]) * (pa[0] - pc_[0]) + (pc_[0] - pb[0]) * (pa[1] - pc_[1])
                if abs(den) < 1e-9:
                    continue
                for gx in range(x0, x1 + 1):
                    for gy in range(y0, y1 + 1):
                        l1 = ((pb[1] - pc_[1]) * (gx - pc_[0]) + (pc_[0] - pb[0]) * (gy - pc_[1])) / den
                        l2 = ((pc_[1] - pa[1]) * (gx - pc_[0]) + (pa[0] - pc_[0]) * (gy - pc_[1])) / den
                        l3 = 1.0 - l1 - l2
                        if l1 < -1e-6 or l2 < -1e-6 or l3 < -1e-6:
                            continue
                        z = l1 * pa[2] + l2 * pb[2] + l3 * pc_[2]
                        o = buf.get((gx, gy))
                        if o is None or z > o[0]:
                            buf[(gx, gy)] = (z, kind, src)
            reach = int(PEEK_REACH / PEEK_CELL)
            def cloth_toward(gx, gy, dx, dy):
                for k in range(1, reach + 1):
                    o = buf.get((gx + dx * k, gy + dy * k))
                    if o is None or o[1] == 2:
                        return False
                    if o[1] == 0:
                        return True
                return False
            hits = [(g, o[2]) for g, o in buf.items() if o[1] == 1 and
                    all(cloth_toward(*g, dx, dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))]
            where = {}
            for _, i in hits:
                w = body_w[i]; k = max((w.items() if isinstance(w, dict) else w), key=lambda kv: kv[1])[0]
                where[lower[k]] = where.get(lower[k], 0) + 1
            area = len(hits) * PEEK_CELL * PEEK_CELL
            out[(name, view)] = (area, sorted(where.items(), key=lambda kv: -kv[1])[:3])
    worst = max(out.items(), key=lambda kv: kv[1][0]) if out else None
    log("peeks (skin through the cloth, cm2 per pose and view): " +
        "  ".join(f"{n}/{v} {a:.1f}" + (f" {j}" if a else "") for (n, v), (a, j) in out.items()))
    return out


UNDER_CM = 10.0      # skin meant to be covered this near a garment is drawn as cloth (under())
UNDER_SHADE = 0.6    # the under layer is the garment's colour this much darker (cloth in its shadow)
UNDER_INSET = 1.5    # cm it sits inside the skin, so a garment over it always shows in front
UNDER_INSET_LOOSE = 2.5   # under a garment with its own weights (the skirt), whose hips move apart from it


def under(body_x, body_w, skip, pieces, lower, force=()):
    """The skin a garment should cover, as the cloth under it: the body points meant to be covered
    (no head's, neck's or hand's weight: OPEN) within UNDER_CM of a garment, hidden or not (skip:
    those left out whole, a robe's legs), are drawn as an under layer the colour of the garment
    they are nearest. Where the cloth parts in a pose (the back of a sleeve as the arm comes down,
    a bulkier body between two pieces) that layer shows, not skin and not a hole into the body.
    force: points drawn so however far from a garment (a robe's shins, as stockings). Returns per
    garment its colour (sRGB, its base image at the nearest points' uvs times its tint) and its
    body points."""
    from PIL import Image
    open_j = {k for k, n in enumerate(lower) if n.split("_")[0] in OPEN}
    gx = [(k, v) for k, pc in enumerate(pieces) for v in pc["verts"]]
    near = Near([v["x"] for _, v in gx], cell=UNDER_CM)
    images = {}
    groups = {}
    for i, (x, ws) in enumerate(zip(body_x, body_w)):
        if i in skip or sum(w for k, w in (ws.items() if isinstance(ws, dict) else ws) if k in open_j) >= 0.2:
            continue
        found = near.nearest(x, UNDER_CM, 128.0)[:1] if i in force else near.within(x, UNDER_CM)
        if not found:
            continue
        k, v = gx[found[0][1]]
        groups.setdefault(k, ([], []))[0].append(i); groups[k][1].append(v["uv"])
    out = []
    for k, (pts, uvs) in groups.items():
        pc = pieces[k]; img = pc["images"][0]
        acc = [0.0, 0.0, 0.0]
        if img is not None:
            if img not in images:
                images[img] = Image.open(img).convert("RGB")
            im = images[img]; W, H = im.size
            for u, v in uvs:
                c = im.getpixel((min(W - 1, max(0, int(u % 1.0 * W))), min(H - 1, max(0, int(v % 1.0 * H)))))
                acc = [a + q for a, q in zip(acc, c)]
            acc = [a / len(uvs) for a in acc]
        else:
            acc = [255.0] * 3
        rgb = tuple(int(round(min(255.0, a * t * UNDER_SHADE))) for a, t in zip(acc, pc["rgb"]))
        out.append((pc["name"], rgb, set(pts), UNDER_INSET_LOOSE if pc["own_weights"] else UNDER_INSET))
    return out


def split_under(tris, groups, pos=lambda i: i):
    """tris (a body's, by vertex; pos: a vertex's body point) split: those with all three points in
    under groups (under()) go to the group most of them are in, the rest stay skin. Returns
    (skin, [tris per group])."""
    skin, out = [], [[] for _ in groups]
    of = {}
    for g, grp in enumerate(groups):
        for q in grp[2]:
            of[q] = g
    for t in tris:
        gs = [of.get(pos(i)) for i in t]
        if None in gs:
            skin.append(t)
        else:
            out[max(set(gs), key=gs.count)].append(t)
    return skin, out


def pack_under(pack, groups, verts, tris, materials, prefix="under"):
    """Each under group's triangles (split_under) as a cloth piece of its colour; verts as packed."""
    for (name, rgb, _, depth), ts in zip(groups, tris):
        if not ts:
            continue
        used = sorted({i for t in ts for i in t}); renum = {i: k for k, i in enumerate(used)}
        key = ((1.0, 1.0, 1.0), 0.9)
        if key not in materials:
            materials[key] = f"cloth{len(materials)}"
            pack.tints[materials[key]] = (1.0, 1.0, 1.0, 0.9)
        inset = [{**verts[i], "p": tuple(p - n * depth / 100.0 for p, n in zip(verts[i]["p"], verts[i]["n"]))} for i in used]
        pack.piece(f"{prefix}/{name}", inset, [tuple(renum[i] for i in t) for t in ts],
                   materials[key], solid(rgb))


def solid(rgb):
    """A small image of one colour (sRGB), for an under layer's base."""
    from PIL import Image
    out = Path.home() / ".openagents/verse/build/cloth"
    out.mkdir(parents=True, exist_ok=True)
    path = out / ("solid-%02x%02x%02x.png" % rgb)
    if not path.exists():
        Image.new("RGB", (8, 8), rgb).save(path)
    return path


def clipped(hair_x, hair_tris, pieces, names):
    """A hood's hair: the triangles of hair_x (cm, the pieces' pose) with every point inside the
    named garments as seen from their middle (the segment from there to the point crosses none
    of their triangles), so hair under a hood shows at its opening and never through it."""
    tris = [tuple(pc["verts"][i]["x"] for i in t) for pc in pieces if pc["name"] in names for t in pc["tris"]]
    if not tris:
        return hair_tris
    pts = [p for t in tris for p in t]
    lo = [min(p[q] for p in pts) for q in range(3)]; hi = [max(p[q] for p in pts) for q in range(3)]
    mid = ((lo[0] + hi[0]) / 2, lo[1] + 0.55 * (hi[1] - lo[1]), (lo[2] + hi[2]) / 2)
    cell = 3.0
    grid = {}
    for k, (a, b, c) in enumerate(tris):
        l = [int(math.floor(min(a[q], b[q], c[q]) / cell)) for q in range(3)]
        h = [int(math.floor(max(a[q], b[q], c[q]) / cell)) for q in range(3)]
        for x in range(l[0], h[0] + 1):
            for y in range(l[1], h[1] + 1):
                for z in range(l[2], h[2] + 1):
                    grid.setdefault((x, y, z), []).append(k)
    inside = []
    for p in hair_x:
        d = sub(p, mid); L = math.sqrt(dot(d, d))
        if L < 1e-6:
            inside.append(True); continue
        u = scale(d, 1.0 / L); seen = set(); hit = False
        steps = int(L / (cell * 0.5)) + 1
        for s_ in range(steps + 1):
            q = add(mid, scale(u, L * s_ / steps))
            for k in grid.get(tuple(int(math.floor(c / cell)) for c in q), ()):
                if k in seen:
                    continue
                seen.add(k)
                t = ray_tri(mid, u, *tris[k])
                if t is not None and 0.0 < t < L:
                    hit = True; break
            if hit:
                break
        inside.append(not hit)
    return [t for t in hair_tris if all(inside[i] for i in t)]


def thighs(leg, body_x, joints, lower, below=15.0):
    """Of a robe's hidden leg points (legs()), the thighs' (from `below` cm under the hip joint down
    to the knee): the under layer leaves those out, so no thigh pokes through the skirt in a
    stride. It keeps the buttocks', under the skirt's waist, and the shins', as stockings from the
    knee down: a boot stepping out behind the hem has its leg."""
    hip = joints[lower.index("thigh_l")][1]; knee = joints[lower.index("calf_l")][1]
    return {i for i in leg if knee < body_x[i][1] < hip - below}


def legs(outfit, body_w, lower):
    """The body points a robe hides whole (outfits' "no_legs"): mostly the thighs' and calves'."""
    if not OUTFITS[outfit].get("no_legs"):
        return set()
    leg = {lower.index(n + sd) for n in ("thigh", "calf") for sd in ("_l", "_r")}
    return {i for i, w in enumerate(body_w) if sum(v for k, v in w.items() if k in leg) >= 0.5}


def ray_tri(o, d, a, b, c):
    """Where the line o + t d meets triangle abc (Moller-Trumbore): t, or None."""
    e1, e2 = sub(b, a), sub(c, a)
    h = cross(d, e2); det = dot(e1, h)
    if abs(det) < 1e-9:
        return None
    f = 1.0 / det; s = sub(o, a)
    u = f * dot(s, h)
    if u < 0.0 or u > 1.0:
        return None
    q = cross(s, e1); v = f * dot(d, q)
    if v < 0.0 or u + v > 1.0:
        return None
    return f * dot(e2, q)


def packed(pack, pieces, materials, log=print):
    """Each fitted piece into the pack (its vertices' "x" in centimetres, the pack's bind pose): a
    cloth material per colour and roughness (materials: key to name, shared with the caller)."""
    for pc in pieces:
        key = (pc["rgb"], round(pc["rough"], 3))
        if key not in materials:
            materials[key] = f"cloth{len(materials)}"
            pack.tints[materials[key]] = (*pc["rgb"], key[1])
        verts = []
        for v in pc["verts"]:
            ws = (list(v["ws"]) + [(0, 0.0)] * 4)[:4]
            verts.append({"p": scale(v["x"], 0.01), "n": v["n"], "uv": tuple(v["uv"]),
                          "j": [k if w > 0 else 0 for k, w in ws], "w": people.quantize_weights([w for _, w in ws])})
        pack.piece(pc["name"], verts, pc["tris"], materials[key], *pc["images"])
    names = sorted({pc["name"] for pc in pieces})
    log(f"outfit: {len(names)} garments packed: {names}")
    return names


def edges(pieces, poses, edge_check, log=print):
    """metahuman.edge_check per piece (positions in cm, the pack's bind pose; lengths as fitted, the
    T pose's "t" when the bind is another); the pieces whose edges spike in idle or walk."""
    bad = []
    for pc in pieces:
        ref = [v["t"] for v in pc["verts"]] if "t" in pc["verts"][0] else None
        res = edge_check([v["x"] for v in pc["verts"]], [v["ws"] for v in pc["verts"]], pc["tris"], poses, ref)
        line = "  ".join(f"{k} {v[0]:.2%}/{v[1]:.0f}x {v[2]:.2%}/{v[3]:.1f}cm" for k, v in res.items())
        log(f"edges {pc['name']} (share past 1.5x / worst ratio, share past 1.5x and 1 cm / most grown): {line}")
        if any(v[2] > 0.005 or v[3] > 4.0 for k, v in res.items() if k.startswith(("idle", "walk"))):
            bad.append(pc["name"])
    return bad
