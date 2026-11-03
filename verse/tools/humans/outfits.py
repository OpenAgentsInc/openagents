"""Fantasy outfits for MetaHuman Lab's human packs: the Quaternius Modular Character Outfits -
Fantasy (CC0; the clothes Everglade's people and 5E Lab's classes wear), fitted onto another body.

    python3 verse/tools/humans/pack_human.py metahuman EXPORT_DIR --outfit fantasy
    python3 verse/tools/humans/pack_human.py standin --outfit fantasy

Outfits (OUTFITS): fantasy (the ranger's tunic, trousers, boots, belt and bracers), hooded (the
same with the hood and pauldron, and no hair under the hood) and robe (the peasant's shirt,
corset and shoes under a long robe, people.robe_mesh(), which hides the legs).

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
    "hooded": {"parts": [("Female_Ranger", RANGER + ["Female_Ranger_Head_Hood", "Female_Ranger_Acc_Pauldrons"])], "hair": False},
    # the peasant's arms end in its own hands (the peasant's image has their skin): cut at the wrist
    # and the robe hides the legs (its own trousers too): no cloth can follow a stride, so a leg
    # would poke through its front; the shoes show under its hem
    "robe": {"parts": [("Female_Peasant", ["Female_Peasant_Arms", "Female_Peasant_Body", "Female_Peasant_Feet"])],
             "hair": True, "no_hands": True, "no_legs": True,
             "robe": {"top": 1.0, "hem": 0.12, "flare": 0.2,
                      "fit": [{"file": f"{OUTFITS_DIR}/Female_Peasant.gltf",
                               "meshes": ["Female_Peasant_Body", "Female_Peasant_Legs", "Female_Peasant_Feet"]}]}},
}
LICENCE = "Quaternius Modular Character Outfits - Fantasy|CC0 1.0"
GAP_MIN = 0.3        # cm: the least cloth stands off the skin
SPREAD_CM = 3.0      # how far a push spreads over the cloth round it
SMALL_CM = 40.0      # a piece no bigger than this every way moves as one
ROBE_SEGMENTS, ROBE_RINGS = 48, 18
ROBE_MARGIN = 0.07   # metres clear of the legs (a crowd's robe: 0.035): mostly the pelvis's, it must clear a stride
ROBE_CINCH_CM, ROBE_WAIST_CM = 14.0, 2.2   # the robe's top 14 cm drawn in to 2.2 cm off the skin at its top edge
ROBE_DYE = (0.16, 0.2, 0.42)   # the robe's colour (its texel is pale cloth, which a crowd dyes per person)
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
                m = mats[pr["material"]]; pbr = m.get("pbrMetallicRoughness", {})
                pieces.append({"name": name, "material": pr["material"], "rgb": tuple(pbr.get("baseColorFactor", (1, 1, 1, 1))[:3]),
                               "rough": pbr.get("roughnessFactor", 1.0),
                               "images": (img(pbr.get("baseColorTexture")), img(m.get("normalTexture")), img(pbr.get("metallicRoughnessTexture"))),
                               "verts": verts(pr), "tris": pr["tris"], "own_weights": False})
    if "robe" in spec:
        # finer than a crowd's robe (people.robe_mesh's 20 x 9): this one is seen close
        keep = people.ROBE_SEGMENTS, people.ROBE_RINGS, people.ROBE_MARGIN
        people.ROBE_SEGMENTS, people.ROBE_RINGS, people.ROBE_MARGIN = ROBE_SEGMENTS, ROBE_RINGS, ROBE_MARGIN
        try:
            robe = people.robe_mesh({"name": "robe", "body": "female", "robe": spec["robe"]}, skel, {})
        finally:
            people.ROBE_SEGMENTS, people.ROBE_RINGS, people.ROBE_MARGIN = keep
        for pr in robe:
            pieces.append({"name": "Robe", "material": pr["material"], "rgb": ROBE_DYE, "rough": 1.0,
                           "images": (pr["image"], None, None), "verts": verts(pr), "tris": pr["tris"], "own_weights": True})
    body = [v for pr in people.part_file({"name": "base", "body": "female"}, BASE, None, skel)
            if pr["material"].startswith("MI_Superhero") for v in verts(pr)]
    joints = [tuple(m[r][3] * 100.0 for r in range(3)) for m in sk.world]
    return pieces, body, joints, sk.parent, lower


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
            # the robe: from the pelvis at the waist, more and more the thighs' down to the knee
            # (ROBE_THIGH of it), split between them smoothly across its whole width (so the
            # stretch between the legs spreads round the robe instead of tearing its front)
            J = target["joints"]
            tl, tr_ = lower.index("thigh_l"), lower.index("thigh_r")
            hip = J[tl][1]; knee = J[lower.index("calf_l")][1]
            mid = (J[tl][0] + J[tr_][0]) / 2; zc = J[pelvis][2]
            for v in pc["verts"]:
                x, y, z = v["x"][0] - mid, v["x"][1], v["x"][2] - zc
                a = ROBE_THIGH * min(1.0, max(0.0, (hip + 4.0 - y) / (hip + 4.0 - knee)))
                sd = 0.5 + 0.5 * x / max(math.hypot(x, z), 1.0)      # by its angle round the hips
                ws = [(pelvis, 1.0 - a), (tl, a * sd), (tr_, a * (1.0 - sd))]
                v["ws"] = sorted([(j, w) for j, w in ws if w > 1e-4], key=lambda kv: -kv[1])
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
    return pieces


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
