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
from mathutils import Vector

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

    After the coordinator's read of round 4 against the reference type (a
    young face): a narrower jaw and lower face, a chin a touch higher (a
    shorter lower face reads younger), fuller lips, higher, more forward
    cheekbones. Gentle and smooth, every edit fading out before the neck
    seam, the ears and the back of the head. These are edits of our own,
    not taken from any person's face."""
    e = (marks.eye_l + marks.eye_r) / 2
    m = marks.mouth
    x, y, z = vs[:, 0], vs[:, 1], vs[:, 2]
    dz = z - e.z
    d = np.zeros_like(vs)
    # Not behind the jaw's angle, the ears or the skull.
    front = 1 - smooth(e.y + 0.05, e.y + 0.085, y)
    # Not into the neck (the seam with the body must not move).
    neck = 1 - smooth(-0.125, -0.165, dz)
    # 1. The lower face narrower: up to 11% in x at the jaw line.
    jaw = smooth(-0.025, -0.085, dz) * neck * front
    d[:, 0] += -x * 0.11 * jaw
    # 2. The chin 5 mm higher, tapering up to the mouth.
    chin = smooth(-0.075, -0.11, dz) * neck * front * np.exp(-(x / 0.035) ** 2)
    d[:, 2] += 0.005 * chin
    # 3. Fuller lips: forward 2.2 mm and 1.4 mm away from the mouth line,
    # in a soft ellipse over the lips.
    lips = np.exp(-(((x - m.x) / 0.026) ** 2) - (((z - m.z) / 0.011) ** 2)) * (y < m.y + 0.02)
    d[:, 1] += -0.0022 * lips
    d[:, 2] += np.sign(z - m.z) * 0.0014 * lips * smooth(0.0, 0.004, np.abs(z - m.z))
    # 4. Higher cheekbones: up 3 mm and forward 2.5 mm under the outer eye.
    for s in (1, -1):
        c = np.array([s * 0.045, e.y - 0.0, e.z - 0.028])
        g = np.exp(-(((x - c[0]) / 0.02) ** 2) - (((y - c[1]) / 0.025) ** 2) - (((z - c[2]) / 0.016) ** 2))
        d[:, 2] += 0.003 * g
        d[:, 1] += -0.0025 * g
    return d


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
    if shape in ("draconia", "fit"):
        rv = np.array([v.co for v in rest.data.vertices])
        if shape == "fit":
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
        marks.profile(rest)
        target = os.environ.get("MH_SHAPE_OUT")
        if target:
            # The shaped rest head in DNA centimetres (x, y up, z forward),
            # for fitting into MetaHuman's identity space in Unreal.
            with open(target, "w") as f:
                for v in rest.data.vertices:
                    f.write(f"{v.co.x * 100:.5f} {v.co.z * 100:.5f} {-v.co.y * 100:.5f}\n")
    t = lambda n: os.path.join(tex_dir, n)  # noqa: E731
    draconia_paint.DETAIL_TILE = (t("T_SkinMicro3_N.tga"), 48, 0.6) if os.path.exists(t("T_SkinMicro3_N.tga")) else None
    draconia_paint.CAVITY_TILE = (t("T_skinMicro3_CAV.tga"), 48) if os.path.exists(t("T_skinMicro3_CAV.tga")) else None
    draconia_paint.BROW_DZ = -0.006
    # The reference's brows are natural and soft-edged, not a drawn band.
    draconia_paint.BROW_FILL = 0.22
    draconia_paint.BROW_STRENGTH = 0.5
    draconia_paint.FOLD_DEPTH = 0.0
    draconia_paint.SKIN_BASE = (0.66, 0.56, 0.54)
    out_dir = os.path.dirname(out)
    if rest is not head:
        # Bake on the rest head alone: the posed one would occlude it.
        bpy.context.collection.objects.unlink(head)
    maps = draconia_paint.paint_skin(rest, marks, size, out_dir)
    if rest is not head:
        bpy.data.objects.remove(rest)
        bpy.context.collection.objects.link(head)
    m = draconia.material("draconia_skin", "skin", images=maps)
    head.data.materials.append(m)

    teeth = draconia.material("mh_teeth", "standard", rough=0.35, images={"base": teeth_rgba(tex_dir, out_dir), "normal": image(t("T_Teeth_Normal.tga"), "Non-Color")})
    objs["teeth"].data.materials.append(teeth)
    eye = draconia.material("mh_eye", "eye", rough=0.15, images={"base": compose_eye(tex_dir, out_dir), "normal": image(t("T_Eye_N.tga"), "Non-Color")}, extras={"verse_shading_params": [0.0, 0.163, 0.0, 0.0]})
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
    draconia_hair.HAIR_ROOT = (0.40, 0.37, 0.30)
    draconia_hair.HAIR_MID = (0.74, 0.71, 0.62)
    draconia_hair.HAIR_TIP = (0.86, 0.84, 0.78)
    draconia_hair.STRIPS_PATH = os.path.join(out_dir, "mh_hair.png")
    draconia_hair.LOCKS_PER_SIDE = 10
    draconia_hair.LOCK_HAIRS = (10, 24)
    draconia_hair.LOCK_LENGTH = (0.1, 0.2)
    # A soft updo, not a frizzed helmet: strands lie close over the scalp
    # and gather back, the hairline comes down the forehead and temples
    # with dense fine roots, and the flyaways are few and fall.
    draconia_hair.LIFT = 0.007
    draconia_hair.PUFF = 0.45
    draconia_hair.MESS = (0.15, 0.5)
    draconia_hair.HAIRLINE_DROP = (0.06, 0.26)
    draconia_hair.EDGE_ROOTS = 5000
    draconia_hair.FLY_COUNT = 260
    draconia_hair.FLY_DROOP = 0.3
    draconia_hair.FLY_LENGTH = (0.03, 0.09)
    # The cap reaches nearly to the hairline in the lengths' shade, so the
    # scalp never shows pale through the strands.
    draconia_hair.CAP_INSET = 0.025
    draconia_hair.CAP_FROM_MID = 0.6
    hair, _ = draconia_hair.grow(head, marks, seed=7, style=HAIR_STYLE)
    extra = list(hair)
    body_dir = os.environ.get("MH_BODY_DIR")
    if body_dir and os.path.exists(os.path.join(body_dir, "body_lod0_mesh.obj")):
        body = read_obj(os.path.join(body_dir, "body_lod0_mesh.obj"), "body_fit")
        extra += draconia_cloth.build(body, marks)
        bpy.data.objects.remove(body)
    # Fewer strands than the CC0 head: the reference's brows are soft, and
    # 650 read as a dark slab at the eye close-up.
    extra += draconia.brows(head, marks, {"l": (None, None, marks.eye_l), "r": (None, None, marks.eye_r)}, count=400)
    parts = list(objs.values()) + extra
    draconia.export(out, parts)
    d = {k: (list(v) if isinstance(v, Vector) else v) for k, v in marks.__dict__.items() if not callable(v)}
    with open(os.path.join(out_dir, "draconia.landmarks.json"), "w") as f:
        json.dump(d, f, indent=1)
    print("METAHUMAN", out)


main()
