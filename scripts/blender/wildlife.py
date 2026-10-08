"""Build Everglade's ambient wildlife and write it as glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/wildlife.py -- [OUT_DIR] [NAME ...]

Writes to OUT_DIR (default: `assets/verse/generated/wildlife`):

- `songbird`, `duck`, and `cat`: generated low-poly animals on simple rigs,
  each solid weighted fully to one bone, as `animals.py` builds Wild Shape's
  beasts. The songbird has `idle` (perched) and `flap`; the duck `idle` and
  `walk` (paddling on the water, its origin at the waterline); the cat
  `idle`, sitting with its tail swaying.
- `rat`, `frog`, `snake`, and `wasp`: lighter copies of the Easy Animated
  Enemy Pack's creatures (CC0, Quaternius), converted by `enemy_pack.py`'s
  `convert` with a smaller triangle budget and only the clips the town
  plays. ENEMY_PACK names the pack's zip (default:
  `~/Downloads/Easy Animated Enemy Pack - Jan 2019.zip`).

Every bone points up +Z from its pivot. Animals face -Y in Blender, which is
+Z (glTF's front) after export; their feet stand at the origin's height.
"""

import math
import os
import sys
import tempfile
import zipfile

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(__file__))
import enemy_pack  # noqa: E402
import kit  # noqa: E402

TAU = 2 * math.pi
PACK = os.environ.get(
    "ENEMY_PACK", os.path.expanduser("~/Downloads/Easy Animated Enemy Pack - Jan 2019.zip")
)

# name: (FBX stem, meters across, triangle budget, {source clip: Verse clip})
AMBIENT = {
    "rat": ("Rat", 0.4, 700, {"Rat_Idle": "idle", "Rat_Walk": "walk", "Rat_Run": "run"}),
    "frog": ("Frog", 0.2, 700, {"Frog_Idle": "idle", "Frog_Jump": "jump"}),
    "snake": ("Snake", 0.7, 700, {"Snake_Idle": "idle", "Snake_Walk": "walk"}),
    "wasp": ("Wasp", 0.2, 500, {"Wasp_Flying": "fly"}),
}


def up(p, h=0.05):
    return (p[0], p[1], p[2] + h)


def rig_and_skin(name, bones, parts):
    body = kit.join(name, parts)
    kit.flat([body])
    arm = kit.armature(name + "_Rig", [(b, parent, head, up(head)) for b, parent, head in bones])
    kit.skin(arm, body)
    return arm


def panel(name, points, thick, material):
    """A thin slab through the outline `points` (x, y, z)."""
    bm = bmesh.new()
    top = [bm.verts.new((x, y, z + thick)) for x, y, z in points]
    bot = [bm.verts.new((x, y, z - thick)) for x, y, z in points]
    bm.faces.new(top)
    bm.faces.new(list(reversed(bot)))
    n = len(points)
    for i in range(n):
        j = (i + 1) % n
        bm.faces.new((top[i], bot[i], bot[j], top[j]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(material)
    return o


def between(name, a, b, r, material, r2=None):
    """A cylinder of radius `r` (tapering to `r2`) from point `a` to `b`."""
    from mathutils import Vector

    a, b = Vector(a), Vector(b)
    d = b - a
    o = kit.cyl(name, r, d.length, tuple((a + b) / 2), material, verts=6, r2=r2 if r2 else r)
    o.rotation_euler = d.to_track_quat("Z", "Y").to_euler()
    return o


def songbird(folder):
    """A swallow-sized songbird: slate back, rust breast, forked tail."""
    kit.reset()
    back = kit.mat("Bird_Back", (0.08, 0.1, 0.18), 0.8)
    breast = kit.mat("Bird_Breast", (0.62, 0.28, 0.12), 0.8)
    beak = kit.mat("Bird_Beak", (0.05, 0.04, 0.03), 0.5)
    hc = 0.09
    parts = [
        kit.bind(kit.ball("Body", 0.5, (0, 0, hc), back, segs=7, rings=5, scale=(0.09, 0.2, 0.09)), "body"),
        kit.bind(kit.ball("Breast", 0.5, (0, -0.03, hc - 0.012), breast, segs=6, rings=4,
                          scale=(0.08, 0.12, 0.075)), "body"),
        kit.bind(kit.ball("Head", 0.045, (0, -0.1, hc + 0.035), back, segs=6, rings=4), "head"),
        kit.bind(kit.cyl("Beak", 0.012, 0.035, (0, -0.15, hc + 0.03), beak, verts=4, r2=0.002,
                         rot=(math.radians(90), 0, 0)), "head"),
    ]
    for sx in (-1, 1):
        parts.append(kit.bind(kit.box("Tail%d" % sx, (0.025, 0.12, 0.008), (sx * 0.018, 0.15, hc + 0.005), back,
                                      rot=(0, 0, sx * math.radians(12))), "tail"))
    bones = [("body", None, (0, 0, hc)), ("head", "body", (0, -0.08, hc + 0.03)), ("tail", "body", (0, 0.09, hc))]
    for side, sx in (("L", 1), ("R", -1)):
        root = (sx * 0.035, -0.02, hc + 0.02)
        bones.append(("wing_" + side, "body", root))
        outline = [(root[0], -0.05, root[2]), (root[0] + sx * 0.24, 0.02, root[2]),
                   (root[0] + sx * 0.2, 0.06, root[2]), (root[0], 0.05, root[2])]
        parts.append(kit.bind(panel("Wing" + side, outline, 0.006, back), "wing_" + side))
    arm = rig_and_skin("Songbird", bones, parts)

    def idle(t):
        s = math.sin(TAU * t)
        fold = math.radians(80)
        return {
            "body": {"rot": (0.04 * s, 0, 0), "loc": (0, 0.003 * (1 + s), 0)},
            "head": (0.15 * math.sin(TAU * 2 * t), 0.6 * math.sin(TAU * t), 0),
            "wing_L": (0, fold, 0),
            "wing_R": (0, -fold, 0),
            "tail": (0.2 * max(0.0, s), 0, 0),
        }

    def flap(t):
        a = math.sin(TAU * t)
        return {
            "body": {"rot": (0, 0, 0), "loc": (0, 0.01 * -a, 0)},
            "head": (0, 0, 0),
            "wing_L": (0, 0, -0.9 * a),
            "wing_R": (0, 0, 0.9 * a),
            "tail": (0.1 * a, 0, 0),
        }

    kit.action(arm, "idle", 8, idle, step=4)
    kit.action(arm, "flap", 6, flap, step=1)
    kit.export(os.path.join(folder, "songbird.glb"), animations=True)


def duck(folder):
    """A drake mallard afloat: green head, white collar, its origin at the waterline."""
    kit.reset()
    brown = kit.mat("Duck_Brown", (0.42, 0.3, 0.2), 0.8)
    grey = kit.mat("Duck_Grey", (0.55, 0.55, 0.52), 0.8)
    green = kit.mat("Duck_Green", (0.03, 0.22, 0.08), 0.4)
    white = kit.mat("Duck_White", (0.9, 0.9, 0.86), 0.8)
    bill = kit.mat("Duck_Bill", (0.85, 0.66, 0.1), 0.6)
    dark = kit.mat("Duck_Dark", (0.04, 0.04, 0.05), 0.5)
    hc = 0.04
    parts = [
        kit.bind(kit.ball("Body", 0.5, (0, 0.02, hc), grey, segs=8, rings=5, scale=(0.22, 0.42, 0.16)), "body"),
        kit.bind(kit.ball("Breast", 0.5, (0, -0.13, hc + 0.01), brown, segs=7, rings=5,
                          scale=(0.2, 0.18, 0.16)), "body"),
        kit.bind(kit.ball("Back", 0.5, (0, 0.06, hc + 0.05), brown, segs=7, rings=4,
                          scale=(0.17, 0.3, 0.08)), "body"),
        kit.bind(kit.box("Tail", (0.08, 0.1, 0.03), (0, 0.24, hc + 0.06), dark,
                         rot=(math.radians(-25), 0, 0)), "tail"),
        kit.bind(kit.cyl("Collar", 0.045, 0.02, (0, -0.17, hc + 0.1), white, verts=8), "head"),
        kit.bind(kit.cyl("Neck", 0.042, 0.1, (0, -0.18, hc + 0.14), green, verts=8), "head"),
        kit.bind(kit.ball("Head", 0.06, (0, -0.2, hc + 0.22), green, segs=7, rings=5,
                          scale=(0.85, 1.15, 0.95)), "head"),
        kit.bind(kit.box("Bill", (0.045, 0.08, 0.02), (0, -0.29, hc + 0.205), bill), "head"),
    ]
    for sx in (-1, 1):
        parts.append(kit.bind(kit.ball("Eye%d" % sx, 0.01, (sx * 0.045, -0.23, hc + 0.235), dark, segs=4, rings=3),
                              "head"))
    bones = [("body", None, (0, 0, hc)), ("head", "body", (0, -0.17, hc + 0.1)), ("tail", "body", (0, 0.2, hc))]
    arm = rig_and_skin("Duck", bones, parts)

    def idle(t):
        s = math.sin(TAU * t)
        dip = max(0.0, math.sin(TAU * 2 * t)) ** 4
        return {
            "body": {"rot": (0.03 * s, 0, 0.02 * math.cos(TAU * t)), "loc": (0, 0.006 * s, 0)},
            "head": (0.25 * dip, 0.7 * math.sin(TAU * t + 0.5), 0),
            "tail": (0, 0.3 * math.sin(TAU * 3 * t), 0),
        }

    def walk(t):
        s = math.sin(TAU * t)
        return {
            "body": {"rot": (0.02 * s, 0, 0.04 * s), "loc": (0, 0.004 * math.cos(TAU * 2 * t), 0)},
            "head": (0.06 * math.sin(TAU * 2 * t), 0.08 * s, 0),
            "tail": (0, 0.25 * s, 0),
        }

    kit.action(arm, "idle", 12, idle, step=6)
    kit.action(arm, "walk", 8, walk, step=3)
    kit.export(os.path.join(folder, "duck.glb"), animations=True)


def cat(folder):
    """A ginger cat sitting upright, its tail curled round and swaying."""
    kit.reset()
    fur = kit.mat("Cat_Ginger", (0.62, 0.3, 0.08), 0.95)
    pale = kit.mat("Cat_Cream", (0.88, 0.76, 0.58), 0.95)
    dark = kit.mat("Cat_Dark", (0.05, 0.04, 0.03), 0.4)
    eye = kit.mat("Cat_Eye", (0.45, 0.55, 0.08), 0.3)
    parts = [
        kit.bind(kit.ball("Haunch", 0.5, (0, 0.04, 0.1), fur, segs=8, rings=5, scale=(0.2, 0.24, 0.2)), "body"),
        kit.bind(kit.ball("Chest", 0.5, (0, -0.04, 0.2), fur, segs=7, rings=5, scale=(0.15, 0.15, 0.26)), "body"),
        kit.bind(kit.ball("Bib", 0.5, (0, -0.1, 0.2), pale, segs=6, rings=4, scale=(0.09, 0.05, 0.16)), "body"),
        kit.bind(kit.ball("Head", 0.075, (0, -0.07, 0.36), fur, segs=8, rings=5, scale=(1.1, 0.95, 0.9)), "head"),
        kit.bind(kit.ball("Muzzle", 0.035, (0, -0.135, 0.34), pale, segs=6, rings=4, scale=(1.3, 0.8, 0.8)), "head"),
        kit.bind(kit.ball("Nose", 0.008, (0, -0.165, 0.35), dark, segs=4, rings=3), "head"),
    ]
    for sx in (-1, 1):
        parts.append(kit.bind(kit.cyl("Ear%d" % sx, 0.03, 0.06, (sx * 0.045, -0.06, 0.43), fur, verts=4, r2=0.003,
                                      rot=(0, sx * math.radians(-15), 0)), "head"))
        parts.append(kit.bind(kit.ball("Eye%d" % sx, 0.012, (sx * 0.032, -0.135, 0.375), eye, segs=4, rings=3),
                              "head"))
        parts.append(kit.bind(kit.cyl("Leg%d" % sx, 0.022, 0.2, (sx * 0.05, -0.1, 0.1), fur, verts=6), "body"))
        parts.append(kit.bind(kit.ball("Paw%d" % sx, 0.03, (sx * 0.05, -0.12, 0.015), pale, segs=5, rings=3,
                                       scale=(1, 1.3, 0.6)), "body"))
    # The tail: off the haunch along the ground, curled round to the front.
    parts.append(kit.bind(between("Tail", (0.02, 0.2, 0.035), (0.15, 0.13, 0.03), 0.022, fur), "tail"))
    parts.append(kit.bind(between("TailTip", (0.15, 0.13, 0.03), (0.17, -0.05, 0.025), 0.02, fur, 0.012),
                          "tail_tip"))
    bones = [("body", None, (0, 0, 0.1)), ("head", "body", (0, -0.06, 0.31)), ("tail", "body", (0.02, 0.2, 0.035)),
             ("tail_tip", "tail", (0.15, 0.13, 0.03))]
    arm = rig_and_skin("Cat", bones, parts)

    def idle(t):
        s = math.sin(TAU * t)
        look = 0.5 * math.sin(TAU * t) if t < 0.5 else 0.15 * math.sin(TAU * 3 * t)
        return {
            "body": {"rot": (0.015 * s, 0, 0), "loc": (0, 0.003 * (1 + s), 0)},
            "head": (0.08 * math.sin(TAU * 2 * t), look, 0),
            "tail": (0, 0, 0.12 * math.sin(TAU * 2 * t)),
            "tail_tip": (0, 0, 0.35 * math.sin(TAU * 2 * t - 0.8)),
        }

    kit.action(arm, "idle", 16, idle, step=6)
    kit.export(os.path.join(folder, "cat.glb"), animations=True)


def ambient(folder, names):
    tmp = tempfile.mkdtemp(prefix="wildlife-")
    with zipfile.ZipFile(PACK) as z:
        for name in names:
            z.extract(enemy_pack.PACK_DIR + AMBIENT[name][0] + ".fbx", tmp)
    for name in names:
        kit.reset()
        stem, meters, budget, clips = AMBIENT[name]
        fbx = os.path.join(tmp, enemy_pack.PACK_DIR, stem + ".fbx")
        enemy_pack.convert(fbx, os.path.join(folder, name + ".glb"), "span", meters, budget, clips)


def main():
    a = kit.args()
    folder = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated", "wildlife")
    os.makedirs(folder, exist_ok=True)
    import coast_wildlife
    built = {"songbird": songbird, "duck": duck, "cat": cat}
    built.update({name: (lambda folder, name=name: coast_wildlife.build(folder, name))
                  for name in coast_wildlife.MODELS})
    names = a[1:] or list(built) + list(AMBIENT)
    for n in names:
        if n in built:
            built[n](folder)
    converted = [n for n in names if n in AMBIENT]
    if converted:
        ambient(folder, converted)


if __name__ == '__main__':
    main()
