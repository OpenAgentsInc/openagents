"""Build training dummies and write them as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/training_dummy.py -- [OUT_DIR]

Writes three variants on one straw dummy (a post on cross feet, a bound
straw body, a sackcloth head, and a crossbar for arms):

- `training_dummy.glb`, the plain straw dummy.
- `training_dummy_armored.glb`, with a breastplate, pauldrons, a helmet,
  and riveted plates on the arms.
- `training_dummy_warded.glb`, with glowing rune bands around the body, a
  sigil on the chest, and a rune circle on the ground. The glow is an
  emissive material, `Dummy_Rune`.

The dummy faces -Y in Blender, which is +Z (glTF's front) after export.
About 1.8 m tall.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402


def straw():
    wood = kit.mat("Dummy_Wood", (0.42, 0.26, 0.12), 0.85)
    hay = kit.mat("Dummy_Straw", (0.86, 0.72, 0.36), 1.0)
    rope = kit.mat("Dummy_Rope", (0.55, 0.42, 0.24), 1.0)
    sack = kit.mat("Dummy_Sack", (0.8, 0.7, 0.52), 1.0)
    # Base: crossed feet and a post.
    for a in (0, 90):
        kit.box("Foot%d" % a, (1.0, 0.14, 0.1), (0, 0, 0.05), wood, rot=(0, 0, math.radians(a + 45)))
    kit.cyl("Post", 0.06, 1.75, (0, 0, 0.875), wood, verts=8)
    # Straw body: a barrel, tied at three heights; straw tufts at its ends.
    kit.lathe(
        "Body",
        [(0, 0.72), (0.2, 0.72), (0.27, 0.82), (0.3, 1.05), (0.29, 1.3), (0.24, 1.44), (0, 1.46)],
        material=hay, segs=12,
    )
    for z, r in ((0.84, 0.275), (1.08, 0.305), (1.32, 0.285)):
        kit.ring("Tie%.2f" % z, r, 0.022, (0, 0, z), rope, segs=12, minor_segs=4)
    kit.cyl("TuftLow", 0.2, 0.12, (0, 0, 0.66), hay, verts=10, r2=0.12)
    # Arms: a crossbar wrapped in straw.
    kit.cyl("Bar", 0.04, 1.2, (0, 0, 1.3), wood, verts=8, rot=(0, math.pi / 2, 0))
    for sx in (-1, 1):
        kit.cyl("Arm%d" % sx, 0.09, 0.32, (sx * 0.42, 0, 1.3), hay, verts=8, rot=(0, math.pi / 2, 0))
        kit.ring("Wrist%d" % sx, 0.09, 0.018, (sx * 0.52, 0, 1.3), rope, segs=8, minor_segs=3, rot=(0, math.pi / 2, 0))
    # Head: a sackcloth ball tied at the neck, with a stitched face.
    kit.ball("Head", 0.19, (0, 0, 1.64), sack, segs=10, rings=7, scale=(1, 0.95, 1.08))
    kit.ring("Neck", 0.09, 0.025, (0, 0, 1.48), rope, segs=10, minor_segs=4)
    stitch = kit.mat("Dummy_Stitch", (0.2, 0.12, 0.08), 1.0)
    for sx in (-1, 1):
        kit.box("Eye%d" % sx, (0.06, 0.02, 0.015), (sx * 0.07, -0.18, 1.68), stitch, rot=(0, math.radians(45 * sx), 0))
        kit.box("EyeX%d" % sx, (0.06, 0.02, 0.015), (sx * 0.07, -0.18, 1.68), stitch, rot=(0, -math.radians(45 * sx), 0))
    kit.box("Mouth", (0.12, 0.02, 0.015), (0, -0.18, 1.58), stitch)
    return wood


def armor():
    iron = kit.mat("Dummy_Iron", (0.45, 0.47, 0.5), 0.4, 0.8)
    trim = kit.mat("Dummy_Brass", (0.75, 0.56, 0.22), 0.4, 0.8)
    leather = kit.mat("Dummy_Leather", (0.3, 0.17, 0.08), 0.9)
    # Breastplate: the front half of a barrel, ridged down the middle.
    kit.lathe(
        "Breastplate",
        [(0, 0.86), (0.32, 0.86), (0.335, 1.05), (0.32, 1.28), (0.26, 1.42), (0, 1.43)],
        material=iron, segs=16,
    )
    kit.ring("Belt", 0.315, 0.035, (0, 0, 0.86), leather, segs=14, minor_segs=4)
    kit.box("Buckle", (0.08, 0.04, 0.07), (0, -0.33, 0.86), trim)
    kit.box("Ridge", (0.04, 0.06, 0.5), (0, -0.325, 1.12), iron, rot=(math.radians(-4), 0, 0))
    for sx in (-1, 1):
        kit.solidify(kit.patch("Pauldron%d" % sx, 0.17, (0, 360), (0, 90), (10, 3), (sx * 0.34, 0, 1.36), iron,
                               scale=(1.2, 1, 0.8)), 0.02)
        kit.ring("PauldronRim%d" % sx, 0.2, 0.02, (sx * 0.34, 0, 1.36), trim, segs=10, minor_segs=3,
                 rot=(0, 0, 0))
        kit.cyl("Bracer%d" % sx, 0.105, 0.22, (sx * 0.47, 0, 1.3), iron, verts=8, rot=(0, math.pi / 2, 0))
        for k in range(3):
            kit.ball("Rivet%d_%d" % (sx, k), 0.018, (sx * 0.14, -0.315, 0.98 + k * 0.14), trim, segs=6, rings=3)
    # Helmet: a dome over the head with a brim and a visor slit.
    kit.solidify(kit.patch("Helm", 0.215, (0, 360), (0, 90), (12, 4), (0, 0, 1.64), iron, scale=(1, 1, 1.1)), 0.02)
    kit.ring("Brim", 0.215, 0.025, (0, 0, 1.64), trim, segs=12, minor_segs=3)
    kit.solidify(kit.patch("Guard", 0.215, (215, 325), (-35, 0), (6, 2), (0, 0, 1.64), iron), 0.02)
    kit.box("Crest", (0.03, 0.3, 0.06), (0, 0, 1.88), trim)
    kit.box("NoseGuard", (0.035, 0.02, 0.14), (0, -0.21, 1.6), iron)


def wards():
    rune = kit.mat("Dummy_Rune", (0.2, 0.7, 1.0), 0.3, emit=(0.15, 0.65, 1.0), strength=1.5)
    stone = kit.mat("Dummy_Stone", (0.4, 0.4, 0.44), 0.9)
    # Rune bands: thin glowing hoops snug on the straw, each with glyph marks.
    for z, r in ((0.95, 0.31), (1.22, 0.31)):
        kit.ring("Band%.2f" % z, r, 0.012, (0, 0, z), rune, segs=16, minor_segs=3)
        for k in range(8):
            a = 2 * math.pi * k / 8
            kit.box("Glyph%.2f_%d" % (z, k), (0.05, 0.015, 0.07), (r * math.cos(a), r * math.sin(a), z + 0.06), rune,
                    rot=(0, 0, a + math.pi / 2))
    # Chest sigil: a ring with a diamond in it.
    kit.ring("Sigil", 0.1, 0.012, (0, -0.305, 1.09), rune, segs=12, minor_segs=3, rot=(math.pi / 2, 0, 0))
    kit.box("SigilCore", (0.08, 0.015, 0.08), (0, -0.31, 1.09), rune, rot=(0, math.pi / 4, 0))
    # Ground circle: a low stone ring with a glowing inlay and four runes.
    kit.lathe("Dais", [(0, 0), (0.85, 0), (0.85, 0.06), (0.78, 0.08), (0, 0.08)], material=stone, segs=20)
    kit.ring("Circle", 0.7, 0.02, (0, 0, 0.085), rune, segs=20, minor_segs=3)
    kit.ring("CircleIn", 0.55, 0.012, (0, 0, 0.085), rune, segs=20, minor_segs=3)
    for k in range(4):
        a = 2 * math.pi * k / 4 + math.pi / 4
        kit.box("Rune%d" % k, (0.1, 0.04, 0.01), (0.625 * math.cos(a), 0.625 * math.sin(a), 0.085), rune,
                rot=(0, 0, a))
        kit.box("RuneBar%d" % k, (0.02, 0.1, 0.01), (0.625 * math.cos(a), 0.625 * math.sin(a), 0.085), rune,
                rot=(0, 0, a + 0.4))
    # The feet stand on the dais.
    for o in kit.meshes():
        if o.name.startswith(("Foot", "Post", "Body", "Tie", "Tuft", "Bar", "Arm", "Wrist", "Head", "Neck", "Eye",
                              "Mouth", "Band", "Glyph", "Sigil")):
            o.location.z += 0.08


def main():
    a = kit.args()
    folder = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated")
    for name, extra in (("training_dummy", None), ("training_dummy_armored", armor), ("training_dummy_warded", wards)):
        kit.reset()
        straw()
        if extra:
            extra()
        body = kit.join(name)
        kit.ground(body)
        kit.flat()
        kit.export(os.path.join(folder, name + ".glb"))


main()
