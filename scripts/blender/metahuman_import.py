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


def compose_eye(tex_dir, out_dir, size=2048, iris_r=0.163, tint=(0.42, 0.55, 0.45)):
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
    col = sample(sclera, u, v)
    k = 0.48 / iris_r
    iu, iv = 0.5 + (u - 0.5) * k, 0.5 + (v - 0.5) * k
    ic = sample(iris, iu, iv)
    lum = ic.mean(-1, keepdims=True)
    green = np.clip(lum * np.array(tint, np.float32) * 2.6, 0, 1)
    # Keep the pupil black and the fibres' contrast.
    green = np.where(lum < 0.08, ic, green)
    t = np.clip(r / iris_r, 0, 2)[..., None]
    limbal = np.clip((t - 0.82) / 0.16, 0, 1)
    green = green * (1 - 0.75 * limbal)
    blend = np.clip((r - iris_r) / 0.006, 0, 1)[..., None]
    col = green * (1 - blend) + col * blend
    rgba = np.concatenate([col, np.ones((size, size, 1), np.float32)], -1)
    return draconia_paint.save("mh_eye_base", rgba, out_dir)


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
        self.chest = (self.eye_l + self.eye_r) / 2 + Vector((0, 0.03, -0.42))
        self.neck = (self.eye_l + self.eye_r) / 2 + Vector((0, 0.02, -0.17))

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
    marks.profile = draconia.Landmarks.profile.__get__(marks)
    marks.profile(head)
    t = lambda n: os.path.join(tex_dir, n)  # noqa: E731
    draconia_paint.DETAIL_TILE = (t("T_SkinMicro3_N.tga"), 48, 0.6) if os.path.exists(t("T_SkinMicro3_N.tga")) else None
    out_dir = os.path.dirname(out)
    maps = draconia_paint.paint_skin(head, marks, size, out_dir)
    m = draconia.material("draconia_skin", "skin", images=maps)
    head.data.materials.append(m)

    teeth = draconia.material("mh_teeth", "standard", rough=0.35, images={"base": image(t("T_Teeth_BaseColor.tga")), "normal": image(t("T_Teeth_Normal.tga"), "Non-Color")})
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

    parts = list(objs.values())
    draconia.export(out, parts)
    d = {k: (list(v) if isinstance(v, Vector) else v) for k, v in marks.__dict__.items() if not callable(v)}
    with open(os.path.join(out_dir, "draconia.landmarks.json"), "w") as f:
        json.dump(d, f, indent=1)
    print("METAHUMAN", out)


main()
