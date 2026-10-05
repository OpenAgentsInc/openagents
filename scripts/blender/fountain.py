"""Build the Fountain Plaza fountain and write it as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/fountain.py -- [OUT.glb | OUT_DIR]

A stepped round basin, a central column carrying two tiered bowls, and a
finial. Water is its own material, so a zone can tint or animate it: the
basin pool, each bowl's pool, the spout, and the sheets spilling over the
bowl rims. The basin is 4.8 m across; the finial stands 3 m up.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

out = kit.out_path("fountain")
kit.reset()

stone = kit.mat("Fountain_Stone", (0.74, 0.7, 0.62), 0.85)
trim = kit.mat("Fountain_Trim", (0.56, 0.53, 0.48), 0.85)
water = kit.mat("Fountain_Water", (0.1, 0.36, 0.62), 0.15)
spill = kit.mat("Fountain_Spill", (0.4, 0.7, 0.9), 0.1)
seg = 24

# Basin: two steps, then a wall with a broad rim, hollow down to its floor.
kit.lathe(
    "Basin",
    [(0, 0), (2.4, 0), (2.4, 0.14), (2.25, 0.14), (2.25, 0.26), (2.12, 0.26), (2.12, 0.62),
     (2.16, 0.66), (2.16, 0.72), (1.9, 0.72), (1.9, 0.3), (0, 0.3)],
    material=stone, segs=seg,
)
kit.cyl("Pool", 1.9, 0.02, (0, 0, 0.56), water, verts=seg)

# Column: a plinth in the pool, a shaft, and the bowls' stems.
kit.lathe(
    "Column",
    [(0, 0.29), (0.5, 0.29), (0.5, 0.5), (0.36, 0.62), (0.26, 0.7), (0.24, 1.25), (0.3, 1.32), (0, 1.32)],
    material=trim, segs=12,
)

# Lower bowl: a shallow dish 1.1 m across the rim.
kit.lathe(
    "BowlLow",
    [(0, 1.25), (0.35, 1.25), (0.9, 1.45), (1.1, 1.62), (1.12, 1.7), (1.0, 1.7), (0.9, 1.6), (0, 1.55)],
    material=stone, segs=seg,
)
kit.cyl("PoolLow", 1.0, 0.02, (0, 0, 1.64), water, verts=seg)


def streams(name, r, top, bottom, count, width):
    """Ribbons of water spilling from a rim at radius `r` down to a pool."""
    for i in range(count):
        a = 2 * math.pi * (i + 0.5) / count
        h = top - bottom
        kit.box(
            "%s%d" % (name, i), (width, 0.025, h), (r * math.cos(a), r * math.sin(a), bottom + h / 2), spill,
            rot=(0, 0, a + math.pi / 2),
        )
        # The lip where it curls over the rim.
        kit.box(
            "%sLip%d" % (name, i), (width, 0.12, 0.03), ((r - 0.05) * math.cos(a), (r - 0.05) * math.sin(a), top),
            spill, rot=(0, 0, a + math.pi / 2),
        )


# Water spilling over the rim in ribbons to the basin pool.
streams("SpillLow", 1.14, 1.7, 0.57, 8, 0.2)

# Upper column and bowl.
kit.lathe("Stem", [(0, 1.6), (0.18, 1.6), (0.14, 1.75), (0.12, 2.2), (0.18, 2.28), (0, 2.28)], material=trim, segs=12)
kit.lathe(
    "BowlHigh",
    [(0, 2.2), (0.2, 2.2), (0.5, 2.32), (0.62, 2.44), (0.63, 2.5), (0.55, 2.5), (0.5, 2.42), (0, 2.38)],
    material=stone, segs=seg,
)
kit.cyl("PoolHigh", 0.55, 0.02, (0, 0, 2.46), water, verts=seg)
streams("SpillHigh", 0.65, 2.5, 1.65, 6, 0.14)

# Finial: a little urn, and the spout rising from it and falling back.
kit.lathe("Finial", [(0, 2.45), (0.1, 2.45), (0.13, 2.6), (0.07, 2.72), (0.09, 2.78), (0, 2.8)], material=trim, segs=10)
kit.cyl("Spout", 0.035, 0.22, (0, 0, 2.9), spill, verts=8, r2=0.015)
kit.lathe("SpoutCrown", [(0, 2.98), (0.12, 2.96), (0.2, 2.86), (0.15, 2.88), (0, 3.04)], material=spill, segs=10)

# Rim posts: eight small pillars around the basin's rim.
for i in range(8):
    a = 2 * math.pi * i / 8 + math.pi / 8
    kit.lathe(
        "Post%d" % i, [(0, 0.72), (0.12, 0.72), (0.09, 0.84), (0.1, 0.9), (0, 0.98)],
        loc=(2.03 * math.cos(a), 2.03 * math.sin(a), 0), material=trim, segs=8,
    )

body = kit.join("Fountain")
kit.ground(body)
kit.flat()
kit.export(out)
