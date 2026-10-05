"""Build market stalls with striped awnings and write them as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/market_stall.py -- [OUT_DIR] [NAME ...]

Writes four color variants, `market_stall_red.glb`, `market_stall_blue.glb`,
`market_stall_green.glb`, and `market_stall_gold.glb` (or only the NAMEs given):
a timber frame, a plank counter with crates and produce, and a sloped
awning of alternating stripes ending in a scalloped valance. The counter
faces -Y in Blender, which is +Z (glTF's front) after export. 2.4 m wide.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

VARIANTS = {
    "market_stall_red": ((0.72, 0.14, 0.12), (0.93, 0.87, 0.72)),
    "market_stall_blue": ((0.16, 0.3, 0.6), (0.93, 0.87, 0.72)),
    "market_stall_green": ((0.16, 0.42, 0.18), (0.93, 0.87, 0.72)),
    "market_stall_gold": ((0.85, 0.6, 0.1), (0.62, 0.16, 0.12)),
}

W, D = 2.4, 1.3  # footprint
POST_FRONT, POST_BACK = 2.15, 2.6  # post heights; the awning slopes forward


def build(name, stripe_a, stripe_b):
    kit.reset()
    wood = kit.mat("Stall_Wood", (0.5, 0.31, 0.15), 0.85)
    dark = kit.mat("Stall_WoodDark", (0.32, 0.19, 0.09), 0.85)
    a = kit.mat("Stall_StripeA", stripe_a, 0.9)
    b = kit.mat("Stall_StripeB", stripe_b, 0.9)
    apple = kit.mat("Stall_Apple", (0.75, 0.12, 0.1), 0.6)
    pear = kit.mat("Stall_Pear", (0.62, 0.72, 0.2), 0.6)
    orange = kit.mat("Stall_Orange", (0.92, 0.5, 0.1), 0.6)
    cloth = kit.mat("Stall_Cloth", stripe_a, 0.95)

    x0, y0 = W / 2 - 0.08, D / 2 - 0.08
    # Posts: taller at the back, so the awning sheds rain to the front.
    for sx in (-1, 1):
        kit.box("PostF%d" % sx, (0.1, 0.1, POST_FRONT), (sx * x0, -y0, POST_FRONT / 2), dark)
        kit.box("PostB%d" % sx, (0.1, 0.1, POST_BACK), (sx * x0, y0, POST_BACK / 2), dark)
    # Counter: a plank top on a boxed front, with a cloth runner.
    kit.box("CounterTop", (W, 0.7, 0.07), (0, -D / 2 + 0.3, 0.92), wood, bevel=0.01)
    kit.box("CounterFront", (W - 0.2, 0.06, 0.86), (0, -D / 2 + 0.03, 0.44), dark)
    for i in range(5):
        kit.box("Plank%d" % i, (0.02, 0.07, 0.84), (-W / 2 + 0.3 + i * 0.45, -D / 2 - 0.0, 0.44), wood)
    kit.box("Runner", (0.7, 0.72, 0.012), (0, -D / 2 + 0.3, 0.962), cloth)
    kit.box("Drape", (0.7, 0.012, 0.3), (0, -D / 2 - 0.05, 0.82), cloth)
    # A back shelf.
    kit.box("Shelf", (W - 0.2, 0.35, 0.05), (0, D / 2 - 0.25, 1.1), wood)

    # Crates on the counter, with fruit heaped in them.
    for k, (cx, fruit) in enumerate(((-0.75, apple), (0.75, orange))):
        kit.box("Crate%d" % k, (0.5, 0.4, 0.2), (cx, -D / 2 + 0.3, 1.05), wood, bevel=0.01)
        for i in range(6):
            fx = cx - 0.15 + (i % 3) * 0.15
            fy = -D / 2 + 0.22 + (i // 3) * 0.16
            kit.ball("Fruit%d_%d" % (k, i), 0.07, (fx, fy, 1.17 + 0.02 * (i % 2)), fruit, segs=8, rings=5)
    for i in range(4):
        kit.ball("Pear%d" % i, 0.06, (-0.15 + i * 0.1, -D / 2 + 0.28, 1.02), pear, segs=8, rings=5, scale=(1, 1, 1.25))
    # Sacks and a crate on the shelf.
    kit.ball("Sack", 0.18, (-0.7, D / 2 - 0.25, 1.3), kit.mat("Stall_Sack", (0.74, 0.62, 0.42), 1.0), segs=8, rings=5,
             scale=(1, 1, 1.2))
    kit.box("ShelfCrate", (0.4, 0.3, 0.25), (0.6, D / 2 - 0.25, 1.25), wood)

    # Awning: stripes running front to back, sloping from the back posts down
    # to the front, overhanging the counter.
    stripes = 8
    over = 0.35
    zb, zf = POST_BACK + 0.04, POST_FRONT - 0.12
    length = math.hypot(D + over, zb - zf)
    slope = math.atan2(zb - zf, D + over)
    cy = (D / 2 + -(D / 2 + over)) / 2
    sw = (W + 0.2) / stripes
    for i in range(stripes):
        x = -(W + 0.2) / 2 + sw * (i + 0.5)
        kit.box("Stripe%d" % i, (sw, length, 0.03), (x, cy, (zb + zf) / 2), a if i % 2 == 0 else b,
                rot=(slope, 0, 0))
        # Scalloped valance: a hanging tab, rounded at the bottom.
        fy = -(D / 2 + over) - 0.01
        kit.box("Tab%d" % i, (sw, 0.02, 0.18), (x, fy, zf - 0.09), a if i % 2 == 0 else b)
        kit.cyl("Scallop%d" % i, sw / 2, 0.02, (x, fy, zf - 0.18), a if i % 2 == 0 else b, verts=10,
                rot=(math.pi / 2, 0, 0))
    # Rafters under the awning and a front beam on the posts.
    kit.box("BeamF", (W, 0.1, 0.1), (0, -y0, POST_FRONT - 0.05), dark)
    kit.box("BeamB", (W, 0.1, 0.1), (0, y0, POST_BACK - 0.05), dark)
    for sx in (-1, 1):
        kit.box("Rafter%d" % sx, (0.08, length, 0.08), (sx * x0, cy, (zb + zf) / 2 - 0.06), dark, rot=(slope, 0, 0))

    body = kit.join(name)
    kit.ground(body)
    kit.flat()
    return body


def main():
    a = kit.args()
    folder = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated")
    names = a[1:] or list(VARIANTS)
    for name in names:
        stripe_a, stripe_b = VARIANTS[name]
        build(name, stripe_a, stripe_b)
        kit.export(os.path.join(folder, name + ".glb"))


main()
