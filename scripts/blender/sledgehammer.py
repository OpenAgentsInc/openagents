"""Build a sledgehammer from code and write it as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/sledgehammer.py -- [OUT.glb | OUT_DIR]

The origin is the handle's butt; the handle runs up +Z (+Y in glTF), 0.9 m,
and the iron head sits across its top along X. The head tapers from a square
eye to octagonal steel striking faces, an iron wedge splits the handle's top,
and leather strips wrap the grip.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

out = kit.out_path("sledgehammer")
kit.reset()

wood = kit.mat("Hammer_Wood", (0.46, 0.27, 0.12), 0.8)
leather = kit.mat("Hammer_Leather", (0.22, 0.11, 0.05), 0.9)
iron = kit.mat("Hammer_Iron", (0.12, 0.13, 0.15), 0.6, 0.35)
steel = kit.mat("Hammer_Steel", (0.62, 0.64, 0.67), 0.3, 0.9)

top = 0.9
# Handle: an oval shaft, thicker at the head and flared at the butt.
kit.cyl("Handle", 0.021, 0.86, (0, 0, 0.45), wood, verts=10, r2=0.024)
kit.cyl("Butt", 0.03, 0.04, (0, 0, 0.02), wood, verts=10, r2=0.022)

# Grip: a leather sleeve and spiral strips over it.
kit.cyl("Grip", 0.026, 0.24, (0, 0, 0.17), leather, verts=10)
for i in range(7):
    z = 0.065 + i * 0.034
    kit.ring("Wrap%d" % i, 0.027, 0.006, (0, 0, z), leather, segs=10, minor_segs=4, rot=(math.radians(14), 0, 0))

# Head: a square eye between two tapered octagonal cheeks.
eye = kit.box("Eye", (0.1, 0.085, 0.1), (0, 0, top), iron, bevel=0.008)
for side in (-1, 1):
    kit.cyl(
        "Cheek%d" % side, 0.058, 0.08, (side * 0.09, 0, top), iron, verts=8, r2=0.05,
        rot=(0, math.radians(90 * side), 0),
    )
    kit.cyl(
        "Face%d" % side, 0.053, 0.022, (side * 0.14, 0, top), steel, verts=8, r2=0.047,
        rot=(0, math.radians(90 * side), 0),
    )
    # A collar where the cheek meets the eye.
    kit.cyl("Collar%d" % side, 0.06, 0.012, (side * 0.055, 0, top), iron, verts=8, rot=(0, math.radians(90), 0))

# Wedge: the handle's end shows through the top of the eye, split by iron.
kit.cyl("HandleEnd", 0.024, 0.014, (0, 0, top + 0.054), wood, verts=10)
kit.box("Wedge", (0.006, 0.05, 0.016), (0, 0, top + 0.06), steel)

kit.join("Sledgehammer")
kit.flat()
kit.export(out)
