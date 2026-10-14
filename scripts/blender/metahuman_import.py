"""A MetaHuman head, from its DNA's meshes, as a Verse glb (private).

Run headless:
    Blender -b --factory-startup --python scripts/blender/metahuman_import.py -- \
        MESH_DIR TEXTURE_DIR OUT.glb [--tex 4096]

MESH_DIR holds `scripts/unreal/dna_dump`'s OBJ files for one level of
detail (head, teeth, saliva, eyeLeft, eyeRight, eyeshell, eyelashes,
eyeEdge, cartilage; neutral, or posed by RigLogic for an expression).
TEXTURE_DIR holds the MetaHuman plugin's own textures
(`scripts/unreal/export_textures.py`), or a DCC export's maps.

Each MetaHuman mesh gets the Verse shading model that matches the role
MetaHuman's own materials give it (`verse_shading` in its glTF extras):

| MetaHuman mesh | Role | Verse shading |
| --- | --- | --- |
| head | skin: albedo, normal (with the tiled micro normal), roughness, cavity | `skin` |
| teeth | teeth and gums | `standard` |
| eyeLeft, eyeRight | sclera and iris, the cornea's bulge in the mesh | `eye` |
| saliva | the wet film inside the lips | `cornea` (specular only) |
| eyeEdge | the tear line on the lid margins | `cornea` |
| eyeshell | the eyes' occlusion shell: soft lid shadow | dropped (Verse bakes it into the eye's occlusion map) |
| eyelashes | lash cards | `hair` |
| cartilage | the hidden inner corner blend | dropped |

The head's albedo is painted by `draconia_paint` on the MetaHuman UVs
until the character's synthesized or downloaded textures exist; the
DCC export's own maps replace it then. Positions are DNA centimetres, Y up,
Z forward; Verse's glTF wants metres, Y up, facing +Z, which is the same
frame scaled.

Everything this reads and writes is a MetaHuman asset: private, never
committed, never used to train AI models.
"""

import json
import math
import os
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(__file__))
import draconia  # noqa: E402
import draconia_paint  # noqa: E402

HAIR_STYLE = os.environ.get("MH_HAIR", "updo")

ROLES = {
    "head": "skin",
    "teeth": "standard",
    "eyeLeft": "eye",
    "eyeRight": "eye",
    "saliva": "cornea",
    "eyeEdge": "cornea",
    "eyelashes": "hair",
}


def read_obj(path, name):
    """An OBJ as a Blender object in Blender's frame (metres, Z up, facing
    -Y): DNA (x, y, z) cm becomes (x, -z, y) / 100."""
    vs, vts, faces = [], [], []
    with open(path) as f:
        for line in f:
            if line.startswith("v "):
                x, y, z = map(float, line.split()[1:4])
                vs.append((x * 0.01, -z * 0.01, y * 0.01))
            elif line.startswith("vt "):
                u, v = map(float, line.split()[1:3])
                vts.append((u, v))
            elif line.startswith("f "):
                idx = [tuple(int(k) - 1 for k in t.split("/")[:2]) for t in line.split()[1:]]
                faces.append(idx)
    me = bpy.data.meshes.new(name)
    me.from_pydata(vs, [], [[p for p, _ in f] for f in faces])
    uv = me.uv_layers.new(name="UVMap")
    k = 0
    for f in faces:
        for _, t in f:
            # DNA texture coordinates have v up, as Blender's.
            uv.data[k].uv = vts[t]
            k += 1
    me.validate()
    for p in me.polygons:
        p.use_smooth = True
    o = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(o)
    return o


def smooth(a, b, x):
    t = np.clip((x - a) / (b - a), 0.0, 1.0)
    return t * t * (3 - 2 * t)


def draconia_shape(vs, marks):
    """Draconia's own face over MetaHuman's head: per-vertex offsets (m,
    Blender frame: x her left, -y forward, z up) computed on the rest head
    and added to every expression's head, so the rig still moves it.

    Round 6 tuned these against the reference's proportions as MetaHuman's
    own tracker measures them (`scripts/verse/draconia_landmarks.py`; the
    numbers are in docs/verse/draconia/parity.md): a shorter upper lip
    (nose base to lip was 27% long), a shorter chin (lower face 8% long),
    rounder, fuller cheeks (the face was 16% narrow at the cheeks), the
    under-eye hollows filled, a fuller upper lip and a slightly narrower
    mouth; the jaw below the mouth stays narrower (round 5). Every edit
    fades out before the neck seam, the ears and the back of the head.
    Proportions only: no person's face is copied."""
    e = (marks.eye_l + marks.eye_r) / 2
    m = marks.mouth
    x, y, z = vs[:, 0], vs[:, 1], vs[:, 2]
    dz = z - e.z
    d = np.zeros_like(vs)
    front = 1 - smooth(e.y + 0.05, e.y + 0.085, y)
    neck = 1 - smooth(-0.125, -0.165, dz)
    # 1. The jaw below the mouth narrower (up to 10%), the cheeks untouched.
    jaw = smooth(-0.06, -0.095, dz) * neck * front
    d[:, 0] += -x * 0.10 * jaw
    # 2. Rounder, fuller cheeks at the nose's base: out (x) and forward,
    # a broad soft bulge over each cheek.
    for s_ in (1, -1):
        c = np.array([s_ * 0.05, e.y + 0.012, e.z - 0.04])
        g = np.exp(-(((x - c[0]) / 0.03) ** 2) - (((y - c[1]) / 0.035) ** 2) - (((z - c[2]) / 0.03) ** 2))
        d[:, 0] += s_ * 0.004 * g
        d[:, 1] += -0.0025 * g
    # 3. Higher cheekbones (round 5).
    for s_ in (1, -1):
        c = np.array([s_ * 0.045, e.y, e.z - 0.028])
        g = np.exp(-(((x - c[0]) / 0.02) ** 2) - (((y - c[1]) / 0.025) ** 2) - (((z - c[2]) / 0.016) ** 2))
        d[:, 2] += 0.002 * g
        d[:, 1] += -0.002 * g
    # 4. The under-eye hollows filled: forward 1.5 mm in a band under each
    # eye.
    for s_ in (1, -1):
        c = np.array([s_ * 0.031, e.y + 0.004, e.z - 0.016])
        g = np.exp(-(((x - c[0]) / 0.017) ** 2) - (((y - c[1]) / 0.02) ** 2) - (((z - c[2]) / 0.008) ** 2))
        d[:, 1] += -0.0015 * g
    # 5. A shorter upper lip and chin: the mouth and everything under it
    # rise 6 mm (fading in from the nose's base), the chin 2.5 mm more.
    # Over the mouth and chin only (a Gaussian across the face), with a
    # broad ramp: a narrow one across the whole face sheared the cheeks
    # into a visible crease.
    across = np.exp(-((x - m.x) / 0.045) ** 2)
    lift = smooth(m.z + 0.03, m.z - 0.005, z) * neck * front * across
    d[:, 2] += 0.006 * lift
    chin = smooth(m.z - 0.01, m.z - 0.05, z) * neck * front * np.exp(-(x / 0.035) ** 2)
    d[:, 2] += 0.0025 * chin
    # 6. Lips: fuller (forward 2 mm), the upper lip taller, the lower lip
    # fuller, the mouth 4% narrower and its corners drawn in.
    # (Soft toward the back: a hard cut here left a seam across the cheeks.)
    lips = np.exp(-(((x - m.x) / 0.026) ** 2) - (((z - m.z) / 0.011) ** 2)) * (1 - smooth(m.y + 0.008, m.y + 0.025, y))
    d[:, 1] += -0.002 * lips
    upper = lips * (z > m.z) * smooth(0.0, 0.004, z - m.z)
    d[:, 2] += 0.0016 * upper
    lower = lips * (z < m.z) * smooth(0.0, 0.004, m.z - z)
    d[:, 2] += -0.0016 * lower
    d[:, 1] += -0.0008 * lower
    d[:, 0] += -(x - m.x) * 0.04 * lips
    # The mouth's corners 2.5 mm in (the tracker measured the mouth 13%
    # wide at 0:24).
    for s_ in (1, -1):
        g = np.exp(-(((x - (m.x + s_ * 0.024)) / 0.011) ** 2) - (((z - m.z) / 0.009) ** 2)) * (1 - smooth(m.y + 0.012, m.y + 0.03, y))
        d[:, 0] += -s_ * 0.0025 * g
    return d


def draconia_refine(vs, marks):
    """Round 7's small edits on top of a blend of MetaHuman presets (the
    coordinator's targets): a narrow V jaw tapering to the chin, a slimmer
    nose with a narrower base, a shorter upper lip, longer eyes. Offsets in
    metres, Blender frame; each fades out before the neck, ears and skull."""
    e = (marks.eye_l + marks.eye_r) / 2
    m = marks.mouth
    nt = marks.nose_tip
    x, y, z = vs[:, 0], vs[:, 1], vs[:, 2]
    d = np.zeros_like(vs)
    front = 1 - smooth(e.y + 0.05, e.y + 0.085, y)
    neck = 1 - smooth(-0.125, -0.165, z - e.z)
    # A V jaw: below the mouth, narrower the lower it goes (to 9% at the
    # chin), so the jaw line tapers.
    v = smooth(m.z - 0.005, m.z - 0.05, z) * neck * front
    d[:, 0] += -x * 0.09 * v
    # A slimmer nose: the nostril wings and the bridge's sides 10% in.
    nose = np.exp(-(((z - (nt.z - 0.008)) / 0.012) ** 2)) * np.exp(-((x / 0.025) ** 2)) * (1 - smooth(nt.y + 0.02, nt.y + 0.035, y))
    d[:, 0] += -x * 0.10 * nose
    # A shorter upper lip: the whole mouth (both lips together) 1 mm up, so
    # the philtrum shortens without the lip growing taller. (A first try
    # lifted the upper lip alone: its vermilion grew 17-22% too tall.)
    ul = np.exp(-(((x - m.x) / 0.03) ** 2) - (((z - m.z) / 0.012) ** 2)) * (1 - smooth(m.y + 0.008, m.y + 0.025, y))
    d[:, 2] += 0.001 * ul
    # Longer, almond eyes (they measured 7% short for their spacing): each
    # eye's outer corner 1.8 mm out and a touch up, the lids following.
    for eye, s_ in ((marks.eye_l, 1.0), (marks.eye_r, -1.0)):
        c = np.array([eye.x + s_ * 0.0165, eye.y + 0.006, eye.z + 0.001])
        g = np.exp(-(((x - c[0]) / 0.008) ** 2) - (((y - c[1]) / 0.01) ** 2) - (((z - c[2]) / 0.007) ** 2))
        d[:, 0] += s_ * 0.0018 * g
        d[:, 2] += 0.0004 * g
    return d


def _umeyama(src, dst):
    """The similarity (s, R, t) taking src onto dst (rows correspond),
    trying both handednesses; returns (apply, median error)."""
    best = None
    for mirror in (1.0, -1.0):
        a0 = src * np.array([mirror, 1.0, 1.0])
        mu_a, mu_b = a0.mean(0), dst.mean(0)
        A, B = a0 - mu_a, dst - mu_b
        U, S, Vt = np.linalg.svd(B.T @ A / len(src))
        D = np.eye(3)
        D[2, 2] = np.sign(np.linalg.det(U @ Vt))
        R = U @ D @ Vt
        s = np.trace(np.diag(S) @ D) / (A**2).sum(1).mean()

        def f(p, mirror=mirror, mu_a=mu_a, mu_b=mu_b, R=R, s=s):
            return (s * (R @ (p * np.array([mirror, 1.0, 1.0]) - mu_a).T)).T + mu_b

        err = np.median(np.linalg.norm(f(src) - dst, axis=1))
        if best is None or err < best[1]:
            best = (f, err)
    return best


def groom_cards(head, arch_obj, groom_dir, grooms, out_dir, colors, push=0.0008):
    """MetaHuman's own hair cards (scripts/unreal/groom_export.py) carried
    onto this head. The cards' FBX export lands in the archetype DNA's own
    frame (centimetres to metres, the same axes: checked against the brows,
    which sit 1.5 cm over the archetype's eyes), so a similarity transform
    takes the archetype head onto this head and each card vertex also
    follows its nearest archetype vertex's residual: the cards sit on this
    face's own shape and expression. Each groom's atlas gives the coverage
    (R) as alpha and its depth (G) as a darker tone in the clumps; `colors`
    is (root, mid, tip) linear, mixed by the atlas's B channel."""
    from mathutils.kdtree import KDTree

    def import_fbx(path):
        """The FBX's meshes with their world transforms baked in and their
        parents removed (deleting a parent first would drop its transform
        from the child)."""
        before = set(bpy.data.objects)
        bpy.ops.import_scene.fbx(filepath=path)
        new = [o for o in bpy.data.objects if o not in before]
        meshes = [o for o in new if o.type == "MESH"]
        for o in meshes:
            mw = o.matrix_world.copy()
            o.parent = None
            o.data.transform(mw)
            o.matrix_world = Matrix()
        for o in new:
            if o.type != "MESH":
                bpy.data.objects.remove(o)
        return meshes

    A = np.array([v.co for v in arch_obj.data.vertices])
    H = np.array([v.co for v in head.data.vertices])
    Hn = np.array([v.normal for v in head.data.vertices])
    from mathutils.bvhtree import BVHTree

    bvh = BVHTree.FromObject(head, bpy.context.evaluated_depsgraph_get())
    to_head, err = _umeyama(A, H)
    resid = H - to_head(A)
    print(f"GROOM archetype to head: {err * 1000:.2f} mm median")
    scalp = KDTree(len(A))
    for i, p in enumerate(A):
        scalp.insert(p, i)
    scalp.balance()
    out = []
    for g in grooms:
        gdir = os.path.join(groom_dir, g)
        if not os.path.isdir(gdir):
            print("GROOM missing", g)
            continue
        atlas = load_px(os.path.join(gdir, f"{g}_CardsAtlas_Attribute.tga"))
        cov, depth, u = atlas[..., 0], atlas[..., 1], atlas[..., 2]
        root, mid, tip = (np.array(c, np.float32) for c in colors)
        t = np.clip(u, 0, 1)[..., None]
        col = np.where(t < 0.3, root + (mid - root) * (t / 0.3), mid + (tip - mid) * (t - 0.3) / 0.7)
        col = col * (0.8 + 0.3 * depth[..., None])
        rgba = np.concatenate([np.clip(col, 0, 1), cov[..., None]], -1)
        img = draconia_paint.save(f"groom_{g}", rgba, out_dir)
        mat = draconia.material(f"groom_{g}", "hair", color=(1, 1, 1, 1), rough=0.4, images={"base": img}, alpha="MASK")
        n = 0
        for fn in sorted(os.listdir(gdir)):
            if not fn.endswith(".fbx"):
                continue
            for o in import_fbx(os.path.join(gdir, fn)):
                if o.name.startswith("UCX_") or not o.data.uv_layers:
                    # (UCX_: Unreal's collision hull, exported alongside.)
                    bpy.data.objects.remove(o)
                    continue
                P = np.array([v.co for v in o.data.vertices])
                near = np.array([scalp.find(p)[1] for p in P])
                # Off the skin by `push` along its normal: the residual
                # mapping is good to about 2 mm, and brow and scalp-hugging
                # cards sit closer than that to the skin (they vanished
                # inside it).
                R_ = to_head(P) + resid[near] + Hn[near] * push
                # Cards that still end up under this head's skin (its crown
                # is shaped differently from the archetype's) come out to
                # just above it, so scalp-hugging grooms don't vanish.
                for v, r in zip(o.data.vertices, R_):
                    q = Vector(r)
                    loc, nrm, _, _ = bvh.find_nearest(q)
                    if loc is not None and (q - loc).dot(nrm) < push:
                        q = loc + nrm * push
                    v.co = q
                o.data.update()
                # The cards' only UV channel is the atlas's (Unreal names it
                # LightMapUV); keep one, named, so the glTF exporter uses it.
                uvs = o.data.uv_layers
                for layer in list(uvs)[1:]:
                    uvs.remove(layer)
                uvs[0].name = "UVMap"
                uvs.active = uvs[0]
                o.data.materials.clear()
                o.data.materials.append(mat)
                o.name = f"groom_{g}_{len(out)}"
                out.append(o)
                n += len(o.data.polygons)
        print("GROOM", g, "card faces", n)
    return out


def fit_deltas(fbx, rest_obj, mesh_dir):
    """Per-vertex offsets from the rest head to MetaHuman Creator's fit of
    Draconia (`scripts/unreal/fit_face.py` exports it as FBX). The FBX
    splits vertices at UV seams and is in Unreal's frame, so its vertices
    are matched to the DNA's by texture coordinate, then aligned to the
    rest head by a similarity transform (the fit's own rigid alignment
    and scale are not ours)."""
    from mathutils.kdtree import KDTree

    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=fbx)
    new = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in new if o.type == "MESH"]
    # LOD0 is the densest.
    fit = max(meshes, key=lambda o: len(o.data.vertices))
    fv = np.array([fit.matrix_world @ v.co for v in fit.data.vertices])
    fuv = np.zeros((len(fv), 2))
    for loop, uvd in zip(fit.data.loops, fit.data.uv_layers.active.data):
        fuv[loop.vertex_index] = uvd.uv
    # The skeletal mesh holds every face part (head, teeth, eyes, lashes)
    # with overlapping UVs: keep the head section's vertices (its material
    # names the head, else the section with the most faces).
    mats = [m.name.lower() if m else "" for m in fit.data.materials]
    counts = np.bincount([p.material_index for p in fit.data.polygons], minlength=len(mats))
    named = [i for i, m in enumerate(mats) if "head" in m]
    head_slot = named[0] if named else int(np.argmax(counts))
    print(f"FIT sections {list(zip(mats, counts.tolist()))}; head is {mats[head_slot] if mats else head_slot}")
    keep = np.zeros(len(fv), bool)
    for p_ in fit.data.polygons:
        if p_.material_index == head_slot:
            keep[list(p_.vertices)] = True
    fv, fuv = fv[keep], fuv[keep]
    for o in new:
        bpy.data.objects.remove(o)
    # The DNA's per-vertex texture coordinates (first seen per position).
    pos, uv = [], []
    vs, vts, pairs = [], [], []
    with open(os.path.join(mesh_dir, "head_lod0_mesh.obj")) as f:
        for line in f:
            if line.startswith("vt "):
                vts.append(tuple(map(float, line.split()[1:3])))
            elif line.startswith("f "):
                for t in line.split()[1:]:
                    a = t.split("/")
                    pairs.append((int(a[0]) - 1, int(a[1]) - 1))
    n = len(rest_obj.data.vertices)
    duv = np.full((n, 2), np.nan)
    for p_, t_ in pairs:
        if np.isnan(duv[p_, 0]):
            duv[p_] = vts[t_]
    rest = np.array([v.co for v in rest_obj.data.vertices])

    def match(fu, fv_):
        tree = KDTree(len(fuv))
        for i, (u, v) in enumerate(fuv):
            tree.insert((1 - u if fu else u, 1 - v if fv_ else v, 0.0), i)
        tree.balance()
        target = np.zeros((n, 3))
        for i in range(n):
            _, j, _ = tree.find((duv[i, 0], duv[i, 1], 0.0))
            target[i] = fv[j]
        return target

    # Similarity transform (Umeyama) taking the fit onto the rest head. Unreal
    # is left-handed, so its FBX may come mirrored: try both handednesses.
    def umeyama(src):
        mu_a, mu_b = src.mean(0), rest.mean(0)
        A, B = src - mu_a, rest - mu_b
        U, S, Vt = np.linalg.svd(B.T @ A / n)
        D = np.eye(3)
        D[2, 2] = np.sign(np.linalg.det(U @ Vt))
        R = U @ D @ Vt
        scale = np.trace(np.diag(S) @ D) / (A ** 2).sum(1).mean()
        out = (scale * (R @ A.T)).T + mu_b
        return np.linalg.norm(out - rest, axis=1), out, scale

    # UV matching is exact once the convention is right (UV gaps are
    # always tiny, so judge each convention by the aligned shape instead).
    best = None
    for fu in (False, True):
        for fv_ in (False, True):
            target = match(fu, fv_)
            for mirror in (1.0, -1.0):
                r = umeyama(target * np.array([mirror, 1.0, 1.0]))
                if best is None or np.median(r[0]) < np.median(best[0]):
                    best = r
                    print(f"FIT convention u-flip {fu} v-flip {fv_} mirror {mirror}: median {np.median(r[0]) * 1000:.2f} mm")
    err, aligned, scale = best
    print(f"FIT matched {n} vertices; mean shift {err.mean() * 1000:.2f} mm, median {np.median(err) * 1000:.2f}, p90 {np.percentile(err, 90) * 1000:.2f}, max {err.max() * 1000:.2f} mm, scale {scale:.4f}")
    return aligned - rest


def load_px(path):
    im = bpy.data.images.load(path)
    w, h = im.size
    a = np.array(im.pixels[:], np.float32).reshape(h, w, im.channels)[::-1]
    bpy.data.images.remove(im)
    return a


def sample(img, u, v):
    """Bilinear-free nearest sample of an image (top row first) at UV
    arrays (v up)."""
    h, w = img.shape[:2]
    x = np.clip((u * w).astype(int), 0, w - 1)
    y = np.clip(((1 - v) * h).astype(int), 0, h - 1)
    return img[y, x]


def compose_eye(tex_dir, out_dir, size=2048, iris_r=0.163, tint=(0.52, 0.62, 0.56)):
    """The eye's albedo on MetaHuman's eye UV, as MetaHuman's eye material
    composes it: the sclera map (veins, the pink back) across the ball, the
    iris photo scaled into the iris disc and turned grey-green (luminance
    times a tint, keeping its fibres), a dark limbal ring at the iris edge,
    and the pupil."""
    sclera = load_px(os.path.join(tex_dir, "T_EyeSclera_D.tga"))[..., :3]
    iris = load_px(os.path.join(tex_dir, "T_Iris001_01_D.tga"))[..., :3]
    yy, xx = np.mgrid[0:size, 0:size].astype(np.float32)
    u = (xx + 0.5) / size
    v = 1 - (yy + 0.5) / size
    r = np.sqrt((u - 0.5) ** 2 + (v - 0.5) ** 2)
    # The sclera a touch brighter than the map and faintly pink, as the
    # reference's at 1:22.
    col = np.clip(sample(sclera, u, v) * np.array([1.09, 1.05, 1.05], np.float32), 0, 1)
    k = 0.48 / iris_r
    iu, iv = 0.5 + (u - 0.5) * k, 0.5 + (v - 0.5) * k
    ic = sample(iris, iu, iv)
    lum = ic.mean(-1, keepdims=True)
    # The fibres' pattern pushed up, so the iris reads textured rather than
    # flat; then a grey-green, mostly grey with a green cast (the
    # reference's iris is light and desaturated, not emerald).
    lum = np.clip(lum + (lum - lum.mean()) * 0.7, 0, 1)
    green = np.clip(lum * np.array(tint, np.float32) * 3.3, 0, 1)
    grey = lum * 3.3 * float(np.mean(tint))
    green = np.clip(green * 0.55 + grey * 0.45, 0, 1)
    # Keep the pupil black and the fibres' contrast.
    green = np.where(lum < 0.08, ic, green)
    t = np.clip(r / iris_r, 0, 2)[..., None]
    limbal = np.clip((t - 0.82) / 0.16, 0, 1)
    green = green * (1 - 0.65 * limbal)
    blend = np.clip((r - iris_r) / 0.006, 0, 1)[..., None]
    col = green * (1 - blend) + col * blend
    rgba = np.concatenate([col, np.ones((size, size, 1), np.float32)], -1)
    return draconia_paint.save("mh_eye_base", rgba, out_dir)


def teeth_rgba(tex_dir, out_dir):
    """The teeth map a little darker and less yellow: inside the mouth they
    sit in the cavity's shade (MetaHuman's teeth material darkens them by
    the mouth's occlusion), and under a warm key the raw map reads gold."""
    t = load_px(os.path.join(tex_dir, "T_Teeth_BaseColor.tga"))
    rgb = t[..., :3]
    lum = rgb.mean(-1, keepdims=True)
    rgb = (lum + (rgb - lum) * 0.55) * 0.8
    return draconia_paint.save("mh_teeth", np.concatenate([rgb, np.ones_like(lum)], -1), out_dir)


def dcc_skin(dcc, maps, out_dir, size):
    """MetaHuman's own skin from the character's DCC export (`Maps/`): its
    albedo as the base colour; its normal (Unreal's Y-down convention,
    flipped to glTF's Y-up) with MetaHuman's tiled micro normal laid over
    for pores; its roughness (SRMF's G) in the ORM's G, keeping our baked
    occlusion in R."""
    base = image(os.path.join(dcc, "Maps", "Head_Basecolor.png"))
    # Creator's palest tone (u 0) is still a saturated peach next to the
    # reference's pale, cool skin; MH_ALBEDO_GRADE "saturation,gain"
    # (default 0.5,1.14: half the saturation, a touch lighter, which also
    # softens its freckles) pulls it toward the reference, on the sRGB
    # values. 1,1 leaves MetaHuman's albedo exactly as exported.
    sat, gain = (float(x) for x in os.environ.get("MH_ALBEDO_GRADE", "0.5,1.14").split(","))
    if (sat, gain) != (1.0, 1.0):
        a = load_px(os.path.join(dcc, "Maps", "Head_Basecolor.png"))
        rgb = a[..., :3]
        lum = rgb.mean(-1, keepdims=True)
        # The lips (the most saturated skin over a broad area) keep more of
        # their colour; chroma is judged blurred, so freckles (small) don't.
        chroma = (rgb.max(-1) - rgb.min(-1)) / np.maximum(rgb.max(-1), 1e-4)
        for _ in range(6):
            chroma = (np.roll(chroma, 8, 0) + np.roll(chroma, -8, 0) + np.roll(chroma, 8, 1) + np.roll(chroma, -8, 1) + chroma) / 5
        keep = np.clip((chroma[..., None] - 0.3) / 0.15, 0, 1)
        k = sat + (min(1.0, sat + 0.4) - sat) * keep
        rgb = np.clip((lum + (rgb - lum) * k) * gain, 0, 1)
        # Soften the freckles (MH_FRECKLE_SOFTEN, default 0.85): a dark
        # spot up to a few millimetres across, darker than its blurred
        # surroundings, is lifted that share toward them. Broad shading is
        # untouched. (Round 7: the reference's freckles are fine and sparse;
        # MetaHuman's read as blotches.)
        soften = float(os.environ.get("MH_FRECKLE_SOFTEN", "0.85"))
        if soften > 0:
            l2 = rgb.mean(-1)
            blur = l2.copy()
            for _ in range(4):
                blur = (np.roll(blur, 6, 0) + np.roll(blur, -6, 0) + np.roll(blur, 6, 1) + np.roll(blur, -6, 1) + blur) / 5
            spot = np.clip(blur - l2, 0, 0.25)[..., None]
            rgb = np.clip(rgb + spot * soften * (rgb / np.maximum(l2[..., None], 1e-4)), 0, 1)
        base = draconia_paint.save("mh_skin_base", np.concatenate([rgb, np.ones_like(lum)], -1), out_dir)
    n = load_px(os.path.join(dcc, "Maps", "Head_Normal.png"))[..., :3] * 2 - 1
    n[..., 1] *= -1
    if draconia_paint.DETAIL_TILE:
        tile_path, repeats, strength = draconia_paint.DETAIL_TILE
        t = load_px(tile_path)[..., :3] * 2 - 1
        h, w = n.shape[:2]
        ys = (np.arange(h) * repeats * t.shape[0] // h) % t.shape[0]
        xs = (np.arange(w) * repeats * t.shape[1] // w) % t.shape[1]
        d = t[ys][:, xs]
        n = np.stack([n[..., 0] + d[..., 0] * strength * 0.6, n[..., 1] + d[..., 1] * strength * 0.6, n[..., 2]], -1)
    n /= np.linalg.norm(n, axis=-1, keepdims=True)
    normal = draconia_paint.save("mh_skin_normal", np.concatenate([n * 0.5 + 0.5, np.ones(n.shape[:2] + (1,), np.float32)], -1), out_dir, "Non-Color")
    srmf = load_px(os.path.join(dcc, "Maps", "Head_SRMF.png"))
    orm_old = np.array(maps["orm"].pixels[:], np.float32).reshape(maps["orm"].size[1], maps["orm"].size[0], 4)[::-1]
    h, w = srmf.shape[:2]
    ys = np.arange(h) * orm_old.shape[0] // h
    xs = np.arange(w) * orm_old.shape[1] // w
    # Our baked occlusion at half strength: at full strength it dug a dark
    # ring into the lids and sockets (round 7), where the reference's lids
    # are clean.
    ao = 1 - (1 - orm_old[ys][:, xs][..., 0]) * 0.5
    rough = srmf[..., 1]
    if rough.max() < 0.01:
        # The UE 5.8 DCC export's SRMF came out empty (all zero); keep the
        # painted roughness rather than read it as a mirror.
        rough = orm_old[ys][:, xs][..., 1]
    orm = np.stack([ao, np.clip(rough, 0.05, 1), np.zeros_like(ao), np.ones_like(ao)], -1)
    return {"base": base, "normal": normal, "orm": draconia_paint.save("mh_skin_orm", orm, out_dir, "Non-Color")}


def lash_rgba(tex_dir, out_dir):
    """Lash cards' colour (near black, brown at the tips) with the coverage
    map as alpha."""
    cov = load_px(os.path.join(tex_dir, "T_Eyelashes_L_Curl_Coverage.tga"))[..., 0]
    h, w = cov.shape
    col = np.zeros((h, w, 4), np.float32)
    col[..., 0], col[..., 1], col[..., 2] = 0.05, 0.04, 0.035
    col[..., 3] = cov
    return draconia_paint.save("mh_lashes", col, out_dir)


def image(path, colorspace="sRGB"):
    if not path or not os.path.exists(path):
        return None
    im = bpy.data.images.load(path)
    im.colorspace_settings.name = colorspace
    return im


class Marks:
    """Landmarks on the MetaHuman head for painting and cameras."""

    def __init__(self, eyes):
        self.eye_l, self.eye_r = eyes
        self.eye_radius = 0.012
        self.mouth = Vector()
        # The neck and the chest where the uniform's cut expects them,
        # measured on the MetaHuman archetype body (1.60 m; its mesh stops
        # at the shoulders, 1.29-1.31 m, under eyes at 1.49 m): the collar
        # sits at the neck's base 0.185 m under the eyes, the chest 0.33 m.
        self.chest = (self.eye_l + self.eye_r) / 2 + Vector((0, 0.035, -0.33))
        self.neck = (self.eye_l + self.eye_r) / 2 + Vector((0, 0.03, -0.165))

    def as_dict(self):
        return {k: list(v) if isinstance(v, Vector) else v for k, v in self.__dict__.items()}


def main():
    argv = sys.argv[sys.argv.index("--") + 1 :]
    mesh_dir, tex_dir, out = argv[0], argv[1], argv[2]
    size = int(argv[argv.index("--tex") + 1]) if "--tex" in argv else 4096
    bpy.ops.wm.read_factory_settings(use_empty=True)
    objs = {}
    for fn in sorted(os.listdir(mesh_dir)):
        if not fn.endswith(".obj"):
            continue
        name = fn.split("_lod")[0]
        if name not in ROLES:
            continue
        objs[name] = read_obj(os.path.join(mesh_dir, fn), name)
    eye_c = []
    for n in ("eyeLeft", "eyeRight"):
        vs = [v.co for v in objs[n].data.vertices]
        eye_c.append(sum(vs, Vector()) / len(vs))
    # DNA "left" is the character's left (+X): eye_l is her left eye.
    marks = Marks(eye_c)
    head = objs["head"]
    # Landmarks and the skin's paint come from the neutral head when this
    # is a posed expression (MH_NEUTRAL_DIR): the UVs are the same, and
    # painting the posed mesh would put the lips' colour where the lips
    # were not at rest.
    neutral_dir = os.environ.get("MH_NEUTRAL_DIR")
    rest = head
    if neutral_dir and os.path.abspath(neutral_dir) != os.path.abspath(mesh_dir):
        rest = read_obj(os.path.join(neutral_dir, "head_lod0_mesh.obj"), "head_rest")
    marks.profile = draconia.Landmarks.profile.__get__(marks)
    marks.profile(rest)
    # Draconia's own shape (MH_SHAPE=draconia, the default: our edits;
    # `fit`: MetaHuman Creator's fit of them; `archetype`: MetaHuman's),
    # computed on the rest head, added to the posed one.
    shape = os.environ.get("MH_SHAPE", "draconia")
    if shape in ("draconia", "fit", "blend"):
        rv = np.array([v.co for v in rest.data.vertices])
        if shape == "blend":
            # A blend of preset heads (MH_BLEND_HEAD: DNA centimetres, one
            # vertex a line, from draconia_preset_blend.py) plus round 7's
            # refinements.
            t = np.loadtxt(os.environ["MH_BLEND_HEAD"]) * 0.01
            target = np.stack([t[:, 0], -t[:, 2], t[:, 1]], 1)
            delta = target - rv
            e = (marks.eye_l + marks.eye_r) / 2
            delta *= (1 - smooth(-0.125, -0.165, rv[:, 2] - e.z))[:, None]
            delta += draconia_refine(rv + delta, marks)
        elif shape == "fit":
            # MetaHuman Creator's fit of our shaped head (MH_FIT_FBX).
            delta = fit_deltas(os.environ["MH_FIT_FBX"], rest, os.environ.get("MH_NEUTRAL_DIR", mesh_dir))
            # The neck stays the archetype's, where it meets the body.
            e = (marks.eye_l + marks.eye_r) / 2
            delta *= (1 - smooth(-0.125, -0.165, rv[:, 2] - e.z))[:, None]
        else:
            delta = draconia_shape(rv, marks)
        for o in {id(rest): rest, id(head): head}.values():
            for v, dv in zip(o.data.vertices, delta):
                v.co += Vector(dv)
            o.data.update()
        # Keep the mouth and nose tip on the same vertices: the profile's
        # geometric search can land elsewhere on a reshaped face (a preset
        # put the painted lips on the philtrum).
        keep = {}
        for k in ("mouth", "nose_tip"):
            if hasattr(marks, k):
                keep[k] = int(np.argmin(np.linalg.norm(rv - np.array(getattr(marks, k)), axis=1)))
        marks.profile(rest)
        for k, i in keep.items():
            setattr(marks, k, rest.data.vertices[i].co.copy())
        target = os.environ.get("MH_SHAPE_OUT")
        if target:
            # The shaped rest head in DNA centimetres (x, y up, z forward),
            # for fitting into MetaHuman's identity space in Unreal.
            with open(target, "w") as f:
                for v in rest.data.vertices:
                    f.write(f"{v.co.x * 100:.5f} {v.co.z * 100:.5f} {-v.co.y * 100:.5f}\n")
    t = lambda n: os.path.join(tex_dir, n)  # noqa: E731
    draconia_paint.DETAIL_TILE = (t("T_SkinMicro3_N.tga"), 48, 1.0) if os.path.exists(t("T_SkinMicro3_N.tga")) else None
    draconia_paint.CAVITY_TILE = (t("T_skinMicro3_CAV.tga"), 48) if os.path.exists(t("T_skinMicro3_CAV.tga")) else None
    draconia_paint.BROW_DZ = -0.006
    # The reference's brows are natural and soft-edged, not a drawn band.
    draconia_paint.BROW_FILL = 0.22
    draconia_paint.BROW_STRENGTH = 0.5
    draconia_paint.FOLD_DEPTH = 0.0
    # Round 6, skin against the reference: warmer, a lighter under-eye,
    # freckles thicker over the nose and cheeks, pores (MetaHuman's micro
    # normal) at full strength.
    draconia_paint.UNDER_EYE = 0.2
    draconia_paint.FRECKLES = (1.35, 0.72)
    # Round 7: a dewy sheen on the forehead and nose, as the reference.
    draconia_paint.TZONE_GLOSS = 0.22
    draconia_paint.SKIN_BASE = (0.68, 0.55, 0.48)
    out_dir = os.path.dirname(out)
    if rest is not head:
        # Bake on the rest head alone: the posed one would occlude it.
        bpy.context.collection.objects.unlink(head)
    maps = draconia_paint.paint_skin(rest, marks, size, out_dir)
    if rest is not head:
        bpy.data.objects.remove(rest)
        bpy.context.collection.objects.link(head)
    # The rigged character's own skin, once Creator has made it (MH_DCC:
    # the DCC export's folder): albedo, normal and roughness replace the
    # painted ones; our baked occlusion stays.
    dcc = os.environ.get("MH_DCC")
    if dcc and os.path.exists(os.path.join(dcc, "Maps", "Head_Basecolor.png")):
        maps = dcc_skin(dcc, maps, out_dir, size)
    # Skin shading: more scattering (warm light through the cheeks and
    # nose) and a stronger, oily specular than the CC0 head's matte 0.3.
    m = draconia.material("draconia_skin", "skin", images=maps, extras={"verse_shading_params": [0.85, 0.85, 0.5, 0.0]})
    head.data.materials.append(m)

    teeth_base = teeth_rgba(tex_dir, out_dir)
    eye_base = None
    if dcc and os.environ.get("MH_DCC_EYES") == "1" and os.path.exists(os.path.join(dcc, "Maps", "Eyes_Color.png")):
        # Creator's eyes and teeth (MH_DCC_EYES=1). Off by default: round 6's
        # export gave a muddy brown iris and yellow teeth next to our
        # composed grey-green iris (MetaHuman's iris photograph, tinted)
        # and toned-down teeth.
        eye_base = image(os.path.join(dcc, "Maps", "Eyes_Color.png"))
        teeth_base = image(os.path.join(dcc, "Maps", "Teeth_Color.png")) or teeth_base
    teeth = draconia.material("mh_teeth", "standard", rough=0.35, images={"base": teeth_base, "normal": image(t("T_Teeth_Normal.tga"), "Non-Color")})
    objs["teeth"].data.materials.append(teeth)
    eye = draconia.material("mh_eye", "eye", rough=0.15, images={"base": eye_base or compose_eye(tex_dir, out_dir), "normal": image(t("T_Eye_N.tga"), "Non-Color")}, extras={"verse_shading_params": [0.0, 0.163, 0.0, 0.0]})
    for n in ("eyeLeft", "eyeRight"):
        objs[n].data.materials.append(eye)
    wet = draconia.material("mh_wet", "cornea", rough=0.08)
    for n in ("saliva", "eyeEdge"):
        if n in objs:
            objs[n].data.materials.append(wet)
    lash = draconia.material("mh_lashes", "hair", color=(1, 1, 1, 1), rough=0.45, images={"base": lash_rgba(tex_dir, out_dir)}, alpha="MASK")
    if "eyelashes" in objs:
        objs["eyelashes"].data.materials.append(lash)

    # Hair: the parity updo grown on this head; the uniform cut over the
    # MetaHuman body (from the body DNA's meshes, when present).
    import draconia_cloth
    import draconia_hair

    # The reference's hair is platinum, near white in the light, with ash
    # roots; and its face-framing locks are thick, falling to the jaw.
    # Platinum ash blonde (round 7: round 6's gold read yellow; round 5's ash
    # read grey): a little warmth in the lengths, near-white tips, darker
    # ash roots.
    draconia_hair.HAIR_ROOT = (0.30, 0.25, 0.18)
    draconia_hair.HAIR_MID = (0.80, 0.74, 0.60)
    draconia_hair.HAIR_TIP = (0.92, 0.89, 0.80)
    draconia_hair.ROOT_SPAN = 0.22
    draconia_hair.STRIPS_PATH = os.path.join(out_dir, "mh_hair.png")
    # Round 6: not two curtains but wisps. More, thinner locks that fall
    # close by the cheeks and curl toward their ends.
    # Round 7: longer, wispier locks that fall past the jaw.
    draconia_hair.LOCKS_PER_SIDE = 12
    draconia_hair.LOCK_HAIRS = (3, 8)
    draconia_hair.LOCK_LENGTH = (0.14, 0.26)
    draconia_hair.LOCK_OUT = 0.3
    draconia_hair.LOCK_CURL = 0.012
    draconia_hair.LOCK_CURL_TURNS = 1.6
    # A loose, messy updo with volume at the crown: strands stand off the
    # scalp more than round 5's (lift 11 mm, puff 0.8), wander more, and
    # gather into the bun without frizz (low wander); wider ribbons so the
    # crown is not grainy.
    # Round 7: looser at the crown, so it isn't a cap (more lift and puff,
    # a little more wander), and a messy centre part.
    draconia_hair.LIFT = 0.016
    draconia_hair.PUFF = 1.1
    draconia_hair.MESS = (0.18, 0.5)
    draconia_hair.PART = 1.3
    draconia_hair.PART_JITTER = 0.006
    draconia_hair.BODY_WIDTH = 1.4
    draconia_hair.HAIRLINE_DROP = (0.06, 0.26)
    draconia_hair.EDGE_ROOTS = 5000
    draconia_hair.FLY_COUNT = 420
    draconia_hair.FLY_DROOP = 0.25
    draconia_hair.FLY_LENGTH = (0.03, 0.1)
    # The cap reaches nearly to the hairline in the roots' shade.
    draconia_hair.CAP_INSET = 0.025
    draconia_hair.CAP_FROM_MID = 0.45
    # MH_FAST=1: the face alone (no hair, uniform or brow strands), for
    # scoring many heads quickly.
    fast = os.environ.get("MH_FAST") == "1"
    # MH_GROOMS ("Hair_S_Updo Eyebrows_M_Natural", from groom_export.py):
    # MetaHuman's own hair cards instead of our grown ribbons; brows among
    # them replace our brow strands. Round 8: warm pale blonde with darker
    # roots; darker ash-brown brows.
    grooms = os.environ.get("MH_GROOMS", "").split()
    groom_objs = []
    if grooms and not fast:
        arch = read_obj(os.path.join(os.environ.get("MH_ARCHETYPE_DIR", os.path.join(os.path.dirname(mesh_dir), "archetype")), "head_lod0_mesh.obj"), "archetype_rest")
        gdir = os.environ.get("MH_GROOM_DIR", os.path.join(os.path.dirname(mesh_dir), "grooms"))
        hair_cols = ((0.28, 0.21, 0.13), (0.80, 0.70, 0.52), (0.90, 0.84, 0.70))
        brow_cols = ((0.045, 0.035, 0.026), (0.07, 0.055, 0.04), (0.10, 0.08, 0.06))
        groom_objs += groom_cards(head, arch, gdir, [g for g in grooms if not g.startswith("Eyebrows")], out_dir, hair_cols, push=0.0025)
        groom_objs += groom_cards(head, arch, gdir, [g for g in grooms if g.startswith("Eyebrows")], out_dir, brow_cols)
        bpy.data.objects.remove(arch)
    if grooms and not fast:
        # With MetaHuman's cards (they cover the scalp and set the
        # hairline): only a few of our face-framing locks, fine and nearly
        # straight, falling past the jaw in the cards' warm blonde.
        draconia_hair.PARTS = ("locks",)
        draconia_hair.LOCK_WAVE = 0.25
        draconia_hair.LOCK_CURL = 0.0
        draconia_hair.LOCKS_PER_SIDE = 5
        draconia_hair.LOCK_HAIRS = (2, 5)
        draconia_hair.HAIR_ROOT, draconia_hair.HAIR_MID, draconia_hair.HAIR_TIP = hair_cols
        hair = groom_objs + draconia_hair.grow(head, marks, seed=7, style=HAIR_STYLE)[0]
    else:
        hair = [] if fast else draconia_hair.grow(head, marks, seed=7, style=HAIR_STYLE)[0]
    extra = list(hair)
    body_dir = None if fast else os.environ.get("MH_BODY_DIR")
    if body_dir and os.path.exists(os.path.join(body_dir, "body_lod0_mesh.obj")):
        body = read_obj(os.path.join(body_dir, "body_lod0_mesh.obj"), "body_fit")
        # MetaHuman's body mesh stops in a low scoop under the neck; the
        # head mesh's lower part (the neck and the top of the chest) fills
        # it. The uniform is cut from both, so it reaches the collar.
        bib = head.copy()
        bib.data = head.data.copy()
        bpy.context.collection.objects.link(bib)
        bm = bmesh.new()
        bm.from_mesh(bib.data)
        cut = marks.neck.z - 0.005
        bmesh.ops.delete(bm, geom=[f for f in bm.faces if f.calc_center_median().z > cut], context="FACES")
        bm.to_mesh(bib.data)
        bm.free()
        bpy.ops.object.select_all(action="DESELECT")
        body.select_set(True)
        bib.select_set(True)
        bpy.context.view_layer.objects.active = body
        bpy.ops.object.join()
        # Round 7: the robe hangs in soft vertical folds.
        draconia_cloth.FOLDS = 0.006
        draconia_cloth.COLLAR_POINTS = 1.0
        extra += draconia_cloth.build(body, marks)
        bpy.data.objects.remove(body)
    # Fewer strands than the CC0 head: the reference's brows are soft, and
    # 650 read as a dark slab at the eye close-up.
    if not fast and not any(g.startswith("Eyebrows") for g in grooms):
        extra += draconia.brows(head, marks, {"l": (None, None, marks.eye_l), "r": (None, None, marks.eye_r)}, count=400)
    parts = list(objs.values()) + extra
    draconia.export(out, parts)
    d = {k: (list(v) if isinstance(v, Vector) else v) for k, v in marks.__dict__.items() if not callable(v)}
    with open(os.path.join(out_dir, "draconia.landmarks.json"), "w") as f:
        json.dump(d, f, indent=1)
    print("METAHUMAN", out)


main()
