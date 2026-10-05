"""The great crypt: a larger, original sibling of the crypt lab's hall, built
for the cultist fight (`verse --crypt-fight`).

Run headless:
    Blender -b --factory-startup --python scripts/blender/great_crypt.py -- \
        [OUT_DIR] [NAME ...]

Reference mode only, as `chamber_lab.py`: the private 1.12.1 Scholomance
laboratory view was studied for the kinds of spaces a cult's crypt holds (a
long vaulted nave, side aisles behind an arcade, side chapels with
sarcophagi, a raised dais at the far end). Nothing here is that geometry,
those textures, or those silhouettes. Every solid is a primitive built in
this file or in `chamber_lab.py`, whose helpers and materials it reuses.

The props inside the crypt (candles, braziers, cauldrons, bookshelves,
cages, chains, bones, cobwebs, sarcophagi) are the crypt lab's own models in
`assets/verse/generated/chamber/`; `verse_world::great_crypt::LAYOUT` places
them. This script writes the hall and four new pieces:

- `great_crypt_hall`: the shell. A nave 10 m wide between two arcades of
  piers, side aisles to walls 21 m apart, a gallery over the aisles, a
  barrel vault over the nave, four side chapels, an entrance landing 1.5 m
  up with stairs down into the nave, and a three-step dais at the far end
  under a tall barred window.
- `summoning_circle`: the glowing circle on the dais (flat, walk-over).
- `broken_pillar`: a snapped pier with its fallen drums (cover).
- `rubble_pile`: collapsed vault stones (cover).

Frame: 1 unit = 1 m. glTF +Y is up and +Z runs toward the entrance; the
hall's origin is the middle of the nave floor. Coordinates below are written
in that glTF frame and converted to Blender's (x, -z, y) where they are
built. The hall's footprint file lists explicit collision boxes, so an
arched opening stays open to a walking character.

Budgets: the hall stays under 90,000 triangles and every piece under 5,000.
"""

import math
import os
import sys

import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402
import chamber_lab as lab  # noqa: E402

# --- The plan, glTF meters (x across, y up, z toward the entrance) ----------------

NAVE = 5.0  # the arcade piers' x
WALL = 10.5  # the aisles' outer wall faces
WALL_T = 0.6
ENTRY = 17.5  # the entrance wall's inner face
FAR = -21.5  # the far wall's inner face
PIERS = (-18.0, -13.5, -9.0, -4.5, 0.0, 4.5, 9.0, 13.5)
PIER_R = 0.45
ARCADE_SPRING = 2.9
ARCADE_TOP = 4.6  # the gallery floor's underside, the aisles' ceiling
GALLERY = 4.8  # the gallery floor's top
PARAPET = 5.8
SPRING = 7.0  # where the nave vault springs
RISE = 3.0
GALLERY_ROOF = 7.6
CHAPELS = (-6.75, 6.75)  # chapel centers along z, both sides
CHAPEL_HALF = 1.7  # half the chapel opening's width
CHAPEL_DEPTH = 3.0
CHAPEL_SPRING = 2.8
CHAPEL_CEILING = 4.6
LANDING = (12.5, 1.5, 4.4)  # front z, top y, half width
STAIRS_HALF = 2.0
TREAD = 0.6
DAIS = ((4.6, -13.0, 0.25), (4.0, -13.8, 0.5), (3.4, -14.6, 0.75))  # half x, front z, top
CIRCLE = (0.0, 0.75, -17.8)
WINDOW = (0.0, 6.6, FAR)  # the far window's center
WINDOW_HALF = 0.7
WINDOW_H = 3.6

NAMES = ["great_crypt_hall", "summoning_circle", "broken_pillar", "rubble_pile"]
BUDGET = {"great_crypt_hall": 90000}
PROP_BUDGET = 5000


def out_dir_and_names():
    argv = kit.args()
    default = os.path.join(kit.REPO, "assets", "verse", "generated", "great_crypt")
    if argv and argv[0] not in NAMES:
        return argv[0], (argv[1:] or NAMES)
    return default, (argv or NAMES)


def B(x, y, z):
    """A glTF point in Blender's frame."""
    return (x, -z, y)


def S(sx, sy, sz):
    """A glTF box size in Blender's frame."""
    return (sx, sz, sy)


def block(name, center, size, material, tile=1.0, rot=(0, 0, 0)):
    """A box from a glTF center and size, with world-projected UVs."""
    obj = kit.box(name, S(*size), B(*center), material, rot=rot)
    lab.uv_box(obj, tile, (center[0] * 0.13 % 1.0, center[2] * 0.17 % 1.0))
    return obj


class Colliders:
    """The collision boxes the footprint file lists, glTF frame."""

    def __init__(self):
        self.boxes = []

    def add(self, name, lo, hi):
        low = [min(a, b) for a, b in zip(lo, hi)]
        high = [max(a, b) for a, b in zip(lo, hi)]
        self.boxes.append(
            {
                "name": name,
                "center": [round((a + b) / 2, 3) for a, b in zip(low, high)],
                "half_extents": [round((b - a) / 2, 3) for a, b in zip(low, high)],
            }
        )


def finish(name, colliders=None, extra=None):
    """Join, write the footprint, export, and hold the budget."""
    lab.shade(kit.meshes())
    sources = list(kit.meshes())
    obj = kit.join(name) if len(sources) > 1 else sources[0]
    if colliders is None:
        kit.ground(obj)
        boxes = [lab.aabb_box(obj, name)]
    else:
        boxes = colliders.boxes
    folder = out_dir_and_names()[0]
    os.makedirs(folder, exist_ok=True)
    data = {
        "model": name + ".glb",
        "frame": "glTF: 1 unit = 1 m, +Y up, +Z toward the entrance, origin at the base center",
        "triangles": kit.triangles(),
        "boxes": boxes,
    }
    if extra:
        data.update(extra)
    with open(os.path.join(folder, name + ".footprint.json"), "w") as handle:
        kit.json.dump(data, handle, indent=2)
        handle.write("\n")
    info = kit.export(os.path.join(folder, name + ".glb"))
    limit = BUDGET.get(name, PROP_BUDGET)
    if info["triangles"] > limit:
        sys.exit("%s is %s triangles; the budget is %s" % (name, info["triangles"], limit))
    return info


def vault_y(x):
    c = max(-1.0, min(1.0, x / NAVE))
    return SPRING + RISE * math.sqrt(max(0.0, 1 - c * c))


# --- The hall --------------------------------------------------------------------------


def floor(m, rng, flags, grout):
    """Grout beds and flagstones over the nave, the aisles, and the chapels."""
    block("floor_bed", (0, -0.12, (ENTRY + FAR) / 2), (WALL * 2 + 1.2, 0.2, ENTRY - FAR + 1.2), grout)
    for z in range(int(FAR), int(ENTRY)):
        cuts = [-WALL + k for k in range(int(WALL * 2) + 1)]
        if z % 2:
            cuts = [-WALL] + [c + 0.5 for c in cuts[:-1]] + [WALL]
        for i, (a, b) in enumerate(zip(cuts, cuts[1:])):
            # The landing covers this part of the nave.
            if z + 0.5 > LANDING[0] and max(abs(a), abs(b)) <= LANDING[2] + 0.6:
                continue
            h = 0.08 + rng.random() * 0.015
            obj = kit.box(
                "flag_%s_%s" % (i, z),
                (b - a - 0.05, 0.95, h),
                B((a + b) / 2, -h / 2 + 0.004 + rng.random() * 0.008, z + 0.5),
                flags[int(rng.random() * 3)],
                rot=(rng.normal() * 0.005, rng.normal() * 0.005, rng.normal() * 0.01),
            )
            lab.uv_box(obj, 1.0, (rng.random(), rng.random()))
    for sx in (-1, 1):
        for cz in CHAPELS:
            block(
                "chapel_floor_%s_%s" % (sx, cz),
                (sx * (WALL + CHAPEL_DEPTH / 2), -0.04, cz),
                (CHAPEL_DEPTH, 0.08, CHAPEL_HALF * 2 + 0.6),
                flags[1],
            )


def side_walls(m, stone, trim, col):
    """The aisles' outer walls with the chapels' arched openings, and the
    chapels behind them."""
    for side, sx in (("east", 1), ("west", -1)):
        x = sx * WALL
        # The prism's frame: origin, u along the wall (Blender +y, which is
        # glTF -z), v up, and the normal out of the room.
        frame = ((x, 0, 0), (0, 1, 0), (0, 0, 1), (sx, 0, 0))
        holes = [(-cz, CHAPEL_HALF, CHAPEL_SPRING) for cz in CHAPELS]
        outline = lab.notched_outline(-ENTRY, -FAR, 0.0, GALLERY_ROOF + 0.1, holes)
        panel = lab.prism("wall_face_%s" % side, outline, WALL_T, stone, frame)
        lab.uv_box(panel, 2.0)
        for cz in CHAPELS:
            for blk in lab.voussoirs("chapel_arch_%s_%s" % (side, cz), -cz, CHAPEL_SPRING, CHAPEL_HALF, frame, trim, count=11, width=0.3):
                lab.uv_box(blk, 1.0)
        # Collision: the wall between the openings, as tall as anything.
        cuts = sorted([cz - CHAPEL_HALF for cz in CHAPELS] + [cz + CHAPEL_HALF for cz in CHAPELS])
        edges = [FAR - 1.0] + cuts + [ENTRY + 1.0]
        for k in range(0, len(edges), 2):
            col.add("wall_%s_%s" % (side, k), (x, -0.5, edges[k]), (x + sx * (WALL_T + 0.4), 30.0, edges[k + 1]))
        # Above each opening, the wall is solid from the arch up.
        for cz in CHAPELS:
            col.add(
                "wall_%s_over_%s" % (side, cz),
                (x, CHAPEL_SPRING + CHAPEL_HALF, cz - CHAPEL_HALF),
                (x + sx * (WALL_T + 0.4), 30.0, cz + CHAPEL_HALF),
            )
        # The chapels: two side walls, a back wall, and a ceiling.
        for cz in CHAPELS:
            x0, x1 = x + sx * WALL_T, x + sx * (WALL_T + CHAPEL_DEPTH)
            for end, z in (("a", cz - CHAPEL_HALF - 0.25), ("b", cz + CHAPEL_HALF + 0.25)):
                block("chapel_side_%s_%s_%s" % (side, cz, end), ((x0 + x1) / 2, CHAPEL_CEILING / 2, z), (abs(x1 - x0), CHAPEL_CEILING, 0.5), stone, 2.0)
                col.add(
                    "wall_chapel_%s_%s_%s" % (side, cz, end),
                    (x0, -0.5, z - 0.25),
                    (x1, 30.0, z + 0.25),
                )
            block("chapel_back_%s_%s" % (side, cz), (x1 + sx * 0.3, CHAPEL_CEILING / 2, cz), (0.6, CHAPEL_CEILING, CHAPEL_HALF * 2 + 1.0), stone, 2.0)
            col.add("wall_chapel_back_%s_%s" % (side, cz), (x1, -0.5, cz - CHAPEL_HALF - 0.5), (x1 + sx * 0.6, 30.0, cz + CHAPEL_HALF + 0.5))
            block("chapel_ceiling_%s_%s" % (side, cz), ((x0 + x1) / 2, CHAPEL_CEILING + 0.15, cz), (abs(x1 - x0) + 0.2, 0.3, CHAPEL_HALF * 2 + 1.0), stone, 2.0)
            col.add("ceiling_chapel_%s_%s" % (side, cz), (x0, CHAPEL_CEILING, cz - CHAPEL_HALF), (x1, CHAPEL_CEILING + 0.3, cz + CHAPEL_HALF))
            # A niche in the chapel's back wall.
            block("chapel_shelf_%s_%s" % (side, cz), (x1 - sx * 0.15, 1.6, cz), (0.3, 0.08, CHAPEL_HALF * 1.4), trim)
        # The string course under the gallery roof.
        block("course_%s" % side, (x - sx * 0.1, GALLERY_ROOF - 1.2, (ENTRY + FAR) / 2), (0.22, 0.24, ENTRY - FAR - 0.4), trim)
        # A low plinth along the wall between the openings.
        for k in range(0, len(edges), 2):
            a, b = max(edges[k], FAR), min(edges[k + 1], ENTRY)
            block("plinth_%s_%s" % (side, k), (x - sx * 0.07, 0.16, (a + b) / 2), (0.14, 0.32, b - a), trim)


def end_walls(m, stone, trim, col):
    """The entrance wall with its door, and the far wall with the window."""
    door_half, door_spring = 1.3, 2.4
    # Entrance (glTF +z): its face looks toward -z. Blender frame: origin on
    # the wall plane, u = +x, v = up, normal out of the room (+z glTF = -y).
    frame = ((0, -ENTRY, 0), (1, 0, 0), (0, 0, 1), (0, -1, 0))
    outline = lab.notched_outline(-WALL, WALL, 0.0, SPRING + RISE + 0.6, [(0.0, door_half, LANDING[1] + door_spring)])
    panel = lab.prism("wall_face_entrance", outline, WALL_T, stone, frame)
    lab.uv_box(panel, 2.0)
    for blk in lab.voussoirs("arch_entrance", 0.0, LANDING[1] + door_spring, door_half, frame, trim, count=11, width=0.32):
        lab.uv_box(blk, 1.0)
    col.add("wall_entrance", (-WALL - 1.0, -0.5, ENTRY), (WALL + 1.0, 30.0, ENTRY + WALL_T + 0.4))
    # The door: two oak leaves under iron straps.
    y = ENTRY + WALL_T * 0.55
    leaf = door_half - 0.01
    door_outline = [(leaf, LANDING[1])] + [(x, z + LANDING[1]) for x, z in lab.arch_points(0.0, door_spring, leaf, leaf, 12)] + [(-leaf, LANDING[1])]
    door = lab.prism("door_leaves", door_outline, 0.14, m.wood, ((0, -y, 0), (1, 0, 0), (0, 0, 1), (0, 1, 0)))
    lab.uv_box(door, 1.6, swap=True)
    for z in (0.5, 1.6, 2.7):
        half = door_half if z < door_spring else math.sqrt(max(0.0, door_half ** 2 - (z - door_spring) ** 2))
        kit.box("door_strap_%s" % z, (half * 2 - 0.1, 0.03, 0.11), B(0, LANDING[1] + z, y - 0.16), m.iron)
    for sx in (-0.3, 0.3):
        kit.ring("door_ring_%s" % sx, 0.1, 0.016, B(sx, LANDING[1] + 1.1, y - 0.2), m.iron, segs=12, minor_segs=4, rot=(math.pi / 2, 0, 0))
    # Far wall (glTF -z), its face toward +z, with a tall lancet window.
    frame = ((0, -FAR, 0), (1, 0, 0), (0, 0, 1), (0, 1, 0))
    wy0 = WINDOW[1] - WINDOW_H / 2
    # The window: the wall built in four parts around it.
    left = [(-WALL, 0.0), (-WINDOW_HALF, 0.0), (-WINDOW_HALF, SPRING + RISE + 0.6), (-WALL, SPRING + RISE + 0.6)]
    right = [(WINDOW_HALF, 0.0), (WALL, 0.0), (WALL, SPRING + RISE + 0.6), (WINDOW_HALF, SPRING + RISE + 0.6)]
    below = [(-WINDOW_HALF, 0.0), (WINDOW_HALF, 0.0), (WINDOW_HALF, wy0), (-WINDOW_HALF, wy0)]
    above = lab.notched_outline(-WINDOW_HALF, WINDOW_HALF, wy0 + WINDOW_H - WINDOW_HALF, SPRING + RISE + 0.6, [(0.0, WINDOW_HALF, wy0 + WINDOW_H - WINDOW_HALF)], segs=8)
    for k, part in enumerate((left, right, below, above)):
        p = lab.prism("wall_face_far_%s" % k, part, WALL_T, stone, frame)
        lab.uv_box(p, 2.0)
    for blk in lab.voussoirs("arch_window", 0.0, wy0 + WINDOW_H - WINDOW_HALF, WINDOW_HALF, frame, trim, count=9, width=0.26):
        lab.uv_box(blk, 1.0)
    col.add("wall_far", (-WALL - 1.0, -0.5, FAR - WALL_T - 0.4), (WALL + 1.0, 30.0, FAR))
    block("window_sill", (0, wy0 - 0.06, FAR - 0.15), (WINDOW_HALF * 2 + 0.4, 0.12, WALL_T + 0.3), trim)
    for bx in (-0.35, 0.0, 0.35):
        lab.rod("window_bar_%s" % bx, B(bx, wy0, FAR - 0.3), B(bx, wy0 + WINDOW_H, FAR - 0.3), 0.03, m.iron, verts=6)
    for by in (wy0 + 1.2, wy0 + 2.4):
        lab.rod("window_rail_%s" % by, B(-WINDOW_HALF, by, FAR - 0.3), B(WINDOW_HALF, by, FAR - 0.3), 0.025, m.iron, verts=6)
    # Moonlit night beyond the bars: a cold, faintly glowing pane set back.
    night = kit.mat("Night_Sky", (0.08, 0.1, 0.16), 0.9, emit=(0.32, 0.42, 0.7), strength=60.0)
    block("window_night", (0, wy0 + WINDOW_H / 2, FAR - WALL_T - 0.05), (WINDOW_HALF * 2 + 0.2, WINDOW_H + 0.4, 0.04), night)
    # Courses where the nave vault springs, on both end walls.
    for z, sz in ((ENTRY - 0.1, 1), (FAR + 0.1, -1)):
        block("course_end_%s" % sz, (0, SPRING - 0.12, z), (NAVE * 2, 0.24, 0.22), trim)


def arcade(m, stone, trim, col):
    """Piers between the nave and the aisles, the arches they carry, the
    gallery over the aisles, and its parapet."""
    for sx in (-1, 1):
        side = "e" if sx > 0 else "w"
        x = sx * NAVE
        for z in PIERS:
            block("pier_plinth_%s_%s" % (side, z), (x, 0.2, z), (1.1, 0.4, 1.1), trim)
            kit.ring("pier_torus_%s_%s" % (side, z), 0.5, 0.08, B(x, 0.46, z), trim, segs=16, minor_segs=5)
            h = ARCADE_SPRING - 0.5
            shaft = kit.cyl("pier_shaft_%s_%s" % (side, z), PIER_R, h, B(x, 0.44 + h / 2, z), stone, verts=16)
            kit.cyl_uv(shaft, 1.5)
            cap = kit.cyl("pier_capital_%s_%s" % (side, z), PIER_R, 0.35, B(x, ARCADE_SPRING - 0.2, z), trim, verts=16, r2=0.68)
            kit.cyl_uv(cap, 1.0)
            block("pier_abacus_%s_%s" % (side, z), (x, ARCADE_SPRING + 0.07, z), (1.4, 0.18, 1.4), trim)
            col.add("pillar_%s_%s" % (side, z), (x - 0.55, -0.5, z - 0.55), (x + 0.55, ARCADE_SPRING + 0.2, z + 0.55))
        # Arches between the piers, as panels with arched holes.
        frame = ((x - sx * 0.35, 0, 0), (0, 1, 0), (0, 0, 1), (sx, 0, 0))
        holes = []
        for a, b in zip(PIERS, PIERS[1:]):
            half = (b - a) / 2 - 0.7
            holes.append((-(a + b) / 2, half, ARCADE_SPRING + 0.16))
        lo, hi = -(PIERS[-1] + 0.7), -(PIERS[0] - 0.7)
        outline = lab.notched_outline(lo, hi, ARCADE_SPRING + 0.16, ARCADE_TOP + 0.2, holes, segs=8)
        spandrel = lab.prism("arcade_%s" % side, outline, 0.7, stone, frame)
        lab.uv_box(spandrel, 2.0)
        for a, b in zip(PIERS, PIERS[1:]):
            half = (b - a) / 2 - 0.7
            for blk in lab.voussoirs("arcade_arch_%s_%s" % (side, a), -(a + b) / 2, ARCADE_SPRING + 0.16, half, frame, trim, count=9, width=0.22, depth=0.05):
                lab.uv_box(blk, 1.0)
        # The gallery: floor slab over the aisle, a parapet on its nave edge,
        # and the roof slab over it.
        zmid = (ENTRY + FAR) / 2
        span = ENTRY - FAR
        block("gallery_floor_%s" % side, (sx * (NAVE + WALL) / 2, (ARCADE_TOP + GALLERY) / 2, zmid), (WALL - NAVE + 0.6, GALLERY - ARCADE_TOP, span), stone, 2.0)
        col.add("ceiling_aisle_%s" % side, (sx * (NAVE - 0.35), ARCADE_TOP, FAR), (sx * WALL, GALLERY, ENTRY))
        block("gallery_parapet_%s" % side, (x, (GALLERY + PARAPET) / 2, (PIERS[0] + PIERS[-1]) / 2), (0.36, PARAPET - GALLERY, PIERS[-1] - PIERS[0] + 1.4), stone, 2.0)
        block("gallery_rail_%s" % side, (x, PARAPET + 0.06, (PIERS[0] + PIERS[-1]) / 2), (0.5, 0.12, PIERS[-1] - PIERS[0] + 1.5), trim)
        col.add("parapet_%s" % side, (x - 0.4, ARCADE_SPRING + 0.16, PIERS[0] - 0.7), (x + 0.4, PARAPET + 0.12, PIERS[-1] + 0.7))
        # Small posts carrying the nave wall above the gallery opening.
        for z in PIERS:
            block("gallery_post_%s_%s" % (side, z), (x, (PARAPET + SPRING) / 2, z), (0.5, SPRING - PARAPET, 0.5), stone)
        block("clerestory_beam_%s" % side, (x, SPRING - 0.15, zmid), (0.6, 0.3, span), trim)
        block("gallery_roof_%s" % side, (sx * (NAVE + WALL) / 2, GALLERY_ROOF + 0.15, zmid), (WALL - NAVE + 0.6, 0.3, span), stone, 2.0)
        col.add("ceiling_gallery_%s" % side, (sx * (NAVE - 0.3), SPRING - 0.3, FAR), (sx * (WALL + 0.6), 30.0, ENTRY))
        # Ends of the arcade, where it meets the end walls above the
        # landing and behind the dais.
        for zend in (PIERS[0] - 0.7, PIERS[-1] + 0.7):
            z1 = FAR if zend < 0 else ENTRY
            block("arcade_end_%s_%s" % (side, zend), (x, (ARCADE_SPRING + SPRING) / 2, (zend + z1) / 2), (0.7, SPRING - ARCADE_SPRING, abs(z1 - zend)), stone, 2.0)


def vault(m, trim, col):
    """The nave's barrel vault, its transverse ribs at the piers, and a
    ridge along the crown."""
    vault_mat = lab.img_mat("Vault_Stone", lab.kit_image("village/T_Brick_BaseColor.png", 256), 0.92, tint=(0.44, 0.41, 0.38))
    segs = 24
    verts = []
    for zz in (FAR - WALL_T, ENTRY + WALL_T):
        for i in range(segs + 1):
            t = math.pi * i / segs
            verts.append(B(NAVE * math.cos(t), SPRING + RISE * math.sin(t), zz))
    faces = [(i, segs + 1 + i, segs + 2 + i, i + 1) for i in range(segs)]
    obj = lab.mesh_obj("vault", verts, faces, vault_mat)
    me = obj.data
    me.update()
    for poly in me.polygons:
        c = poly.center
        inward = Vector((-c.x / NAVE ** 2, 0, -(c.z - SPRING) / RISE ** 2))
        if poly.normal.dot(inward) < 0:
            poly.flip()
    me.update()
    arc = [0.0]
    for i in range(1, segs + 1):
        arc.append(arc[-1] + (Vector(verts[i]) - Vector(verts[i - 1])).length)
    layer = me.uv_layers.new(name="UVMap")
    for poly in me.polygons:
        for li in poly.loop_indices:
            vi = me.loops[li].vertex_index
            layer.data[li].uv = (verts[vi][1] / 2.0, arc[vi % (segs + 1)] / 2.0)
    sol = obj.modifiers.new("Solidify", "SOLIDIFY")
    sol.thickness = 0.4
    sol.offset = -1.0
    col.add("ceiling_nave", (-NAVE - 0.5, SPRING + RISE - 1.1, FAR - 1.0), (NAVE + 0.5, 30.0, ENTRY + 1.0))
    for z in PIERS:
        pts = [(NAVE * math.cos(math.pi * i / 16), SPRING + RISE * math.sin(math.pi * i / 16)) for i in range(17)]
        for i, (a, b) in enumerate(zip(pts, pts[1:])):
            p0 = Vector(B(a[0], a[1], z))
            p1 = Vector(B(b[0], b[1], z))
            mid = (p0 + p1) / 2
            inward = Vector((-mid.x / NAVE ** 2, 0, -(mid.z - SPRING) / RISE ** 2)).normalized()
            seg = kit.box("rib_%s_%s" % (z, i), ((p1 - p0).length + 0.05, 0.4, 0.3), (0, 0, 0), trim)
            along = (p1 - p0).normalized()
            basis = Matrix((along, Vector((0, 1, 0)), -inward)).transposed().to_4x4()
            seg.matrix_world = Matrix.Translation(mid + inward * 0.12) @ basis
            lab.uv_box(seg, 1.0)
        kit.ball("boss_%s" % z, 0.24, B(0, SPRING + RISE - 0.32, z), trim, segs=10, rings=6, scale=(1, 1, 0.6))
    block("ridge", (0, SPRING + RISE - 0.1, (ENTRY + FAR) / 2), (0.3, 0.24, ENTRY - FAR), trim)


def landing(m, stone, trim, col):
    """The entrance landing, its parapets, and the stairs down."""
    front, top, half = LANDING
    block("landing", (0, top / 2, (front + ENTRY) / 2), (half * 2, top, ENTRY - front), stone, 2.0)
    col.add("landing", (-half, -0.5, front), (half, top, ENTRY + 0.2))
    block("landing_lip", (0, top + 0.03, front + 0.1), (half * 2 + 0.06, 0.08, 0.26), trim)
    # Steps down into the nave.
    count = int(round(top / 0.25))
    for i in range(1, count):
        h = 0.25 * i
        z0 = front - TREAD * (count - i)
        z1 = z0 + TREAD
        block("stair_%s" % i, (0, h / 2, (z0 + z1) / 2), (STAIRS_HALF * 2, h, TREAD), trim)
        block("stair_nose_%s" % i, (0, h + 0.02, z0 + 0.05), (STAIRS_HALF * 2 + 0.04, 0.06, 0.12), trim)
        col.add("stair_%s" % i, (-STAIRS_HALF, -0.5, z0), (STAIRS_HALF, h, z1))
    # Stringer walls on both sides of the stairs, below the landing parapet.
    z_bottom = front - TREAD * (count - 1)
    for sx in (-1, 1):
        x = sx * (STAIRS_HALF + 0.15)
        # The prism's u is Blender +y, which is glTF -z.
        outline = [(-front, 0.0), (-front, top + 0.55), (-z_bottom, 0.55), (-z_bottom, 0.0)]
        stringer = lab.prism("stair_side_%s" % sx, outline, 0.3, trim, ((x - 0.15, 0, 0), (0, 1, 0), (0, 0, 1), (1, 0, 0)))
        lab.uv_box(stringer, 1.0)
        col.add("stair_side_%s" % sx, (x - 0.15, -0.5, z_bottom), (x + 0.15, top + 0.55, front))
        # The landing's parapet beside the stairs and along its sides.
        cx = sx * (STAIRS_HALF + half) / 2
        block("landing_parapet_front_%s" % sx, (cx, top + 0.45, front + 0.15), (half - STAIRS_HALF, 0.9, 0.3), stone)
        block("landing_parapet_front_cap_%s" % sx, (cx, top + 0.93, front + 0.15), (half - STAIRS_HALF + 0.1, 0.08, 0.4), trim)
        col.add("parapet_landing_front_%s" % sx, (sx * STAIRS_HALF, top, front), (sx * half, top + 0.95, front + 0.3))
        block("landing_parapet_side_%s" % sx, (sx * (half - 0.15), top + 0.45, (front + ENTRY) / 2), (0.3, 0.9, ENTRY - front), stone)
        block("landing_parapet_side_cap_%s" % sx, (sx * (half - 0.15), top + 0.93, (front + ENTRY) / 2), (0.4, 0.08, ENTRY - front + 0.1), trim)
        col.add("parapet_landing_side_%s" % sx, (sx * (half - 0.3), top, front), (sx * half, top + 0.95, ENTRY))
        # A newel post at the stair head.
        block("newel_%s" % sx, (sx * (STAIRS_HALF + 0.15), top + 0.55, front + 0.15), (0.4, 1.1, 0.4), trim)


def dais(m, stone, trim, col):
    """Three steps up to the summoning circle, under the far window."""
    for k, (half, front, top) in enumerate(DAIS):
        block("dais_%s" % k, (0, top / 2, (front + FAR) / 2), (half * 2, top, front - FAR), stone if k == 0 else trim, 1.5)
        block("dais_edge_%s" % k, (0, top + 0.02, front - 0.06), (half * 2 + 0.04, 0.05, 0.14), trim)
        col.add("dais_%s" % k, (-half, -0.5, FAR - 0.1), (half, top, front))
    # Obelisks at the dais's back corners.
    for sx in (-1, 1):
        x = sx * (DAIS[2][0] - 0.5)
        block("obelisk_base_%s" % sx, (x, DAIS[2][2] + 0.2, FAR + 0.6), (0.8, 0.4, 0.8), trim)
        ob = kit.cyl("obelisk_%s" % sx, 0.28, 3.2, B(x, DAIS[2][2] + 0.4 + 1.6, FAR + 0.6), m.stone, verts=4, r2=0.12, rot=(0, 0, math.pi / 4))
        lab.uv_box(ob, 1.0)
        col.add("obelisk_%s" % sx, (x - 0.4, -0.5, FAR + 0.2), (x + 0.4, DAIS[2][2] + 3.6, FAR + 1.0))


def rubble_along_walls(stone, rng):
    for k in range(26):
        sx = 1 if k % 2 else -1
        z = FAR + 1.0 + rng.random() * (ENTRY - FAR - 2.0)
        if any(abs(z - cz) < CHAPEL_HALF + 0.3 for cz in CHAPELS):
            continue
        s = 0.12 + rng.random() * 0.16
        obj = kit.box(
            "rubble_%s" % k,
            (s * 1.4, s, s * 0.8),
            B(sx * (WALL - 0.3 - rng.random() * 0.2), s * 0.3, z),
            stone,
            rot=(rng.random() * 0.4, rng.random() * 0.4, rng.random() * 3),
        )
        lab.uv_box(obj, 1.0)


def build_hall():
    kit.reset()
    m = lab.Mats()
    rng = lab.rng_for("great_crypt_hall")
    flag_img = lab.to_image("T_Lab_Flag", lab.tex_flag())
    flags = [
        lab.img_mat("Flag_A", flag_img, 0.88, tint=(1.0, 1.0, 1.0)),
        lab.img_mat("Flag_B", flag_img, 0.9, tint=(0.8, 0.78, 0.74)),
        lab.img_mat("Flag_C", flag_img, 0.86, tint=(0.93, 0.86, 0.78)),
    ]
    grout = kit.mat("Grout", (0.05, 0.045, 0.04), 0.95)
    col = Colliders()
    col.add("floor", (-WALL - CHAPEL_DEPTH - 1.0, -0.6, FAR - 1.0), (WALL + CHAPEL_DEPTH + 1.0, 0.0, ENTRY + 1.0))
    floor(m, rng, flags, grout)
    side_walls(m, m.stone, m.stone_trim, col)
    end_walls(m, m.stone, m.stone_trim, col)
    arcade(m, m.stone, m.stone_trim, col)
    vault(m, m.stone_trim, col)
    landing(m, m.stone, m.stone_trim, col)
    dais(m, m.stone, m.stone_trim, col)
    rubble_along_walls(m.stone, rng)
    extra = {
        "plan": {
            "nave_half_width": NAVE,
            "wall": WALL,
            "entrance": ENTRY,
            "far": FAR,
            "landing": {"front": LANDING[0], "top": LANDING[1], "half_width": LANDING[2]},
            "spring": SPRING,
            "rise": RISE,
        }
    }
    return finish("great_crypt_hall", colliders=col, extra=extra)


# --- The pieces -------------------------------------------------------------------------


def build_summoning_circle():
    """Two glowing rings, a seven-pointed star between them, and runes,
    laid flat on the dais. Walk-over: it has no height."""
    kit.reset()
    glow = kit.mat("Ritual_Glow", (0.55, 0.2, 0.9), 0.4, emit=(0.62, 0.16, 1.0), strength=900.0)
    ember = kit.mat("Ritual_Ember", (0.9, 0.25, 0.5), 0.4, emit=(1.0, 0.22, 0.55), strength=600.0)
    scorch = kit.mat("Ritual_Scorch", (0.03, 0.02, 0.025), 0.95)
    r = 2.2
    kit.cyl("Scorch", r + 0.25, 0.006, (0, 0, 0.003), scorch, verts=48)
    kit.ring("Outer", r, 0.035, (0, 0, 0.012), glow, segs=64, minor_segs=4)
    kit.ring("Inner", r * 0.78, 0.028, (0, 0, 0.012), glow, segs=56, minor_segs=4)
    kit.ring("Core", r * 0.22, 0.03, (0, 0, 0.012), ember, segs=24, minor_segs=4)
    points = [(r * 0.78 * math.cos(math.pi / 2 + k * math.tau / 7), r * 0.78 * math.sin(math.pi / 2 + k * math.tau / 7)) for k in range(7)]
    for k in range(7):
        a = points[k]
        b = points[(k + 3) % 7]
        lab.rod("Star%s" % k, (a[0], a[1], 0.012), (b[0], b[1], 0.012), 0.022, glow, verts=4)
    rng = lab.rng_for("summoning_circle")
    for k in range(28):
        t = k * math.tau / 28
        rr = r * 0.89
        kind = int(rng.random() * 3)
        size = 0.09
        x, y = rr * math.cos(t), rr * math.sin(t)
        if kind == 0:
            lab.rod("Rune%s_a" % k, (x - size * math.sin(t), y + size * math.cos(t), 0.012), (x + size * math.sin(t), y - size * math.cos(t), 0.012), 0.012, ember, verts=4)
        elif kind == 1:
            kit.ring("Rune%s_o" % k, size * 0.6, 0.012, (x, y, 0.012), ember, segs=8, minor_segs=3)
        else:
            lab.rod("Rune%s_v1" % k, (x, y, 0.012), (x + size * math.cos(t + 0.6), y + size * math.sin(t + 0.6), 0.012), 0.012, ember, verts=4)
            lab.rod("Rune%s_v2" % k, (x, y, 0.012), (x + size * math.cos(t - 0.6), y + size * math.sin(t - 0.6), 0.012), 0.012, ember, verts=4)
    for o in kit.meshes():
        if not o.data.uv_layers:
            o.data.uv_layers.new(name="UVMap")
    return finish("summoning_circle")


def build_broken_pillar():
    """A pier snapped at shoulder height, its fallen drums beside it."""
    kit.reset()
    m = lab.Mats()
    rng = lab.rng_for("broken_pillar")
    kit.box("Plinth", (1.1, 1.1, 0.4), (0, 0, 0.2), m.stone_trim)
    kit.ring("Torus", 0.5, 0.08, (0, 0, 0.46), m.stone_trim, segs=16, minor_segs=5)
    shaft = kit.cyl("Shaft", PIER_R, 1.6, (0, 0, 0.44 + 0.8), m.stone, verts=16)
    # The snapped top: a jagged cap of small wedges.
    for k in range(9):
        a = k * math.tau / 9
        h = 0.08 + rng.random() * 0.3
        kit.cyl("Jag%s" % k, 0.16, h, (0.3 * math.cos(a), 0.3 * math.sin(a), 2.04 + h / 2), m.stone, verts=4, r2=0.02, rot=(rng.random() * 0.3, rng.random() * 0.3, a))
    for k, (x, y, yaw) in enumerate(((1.15, 0.3, 0.4), (0.9, -0.85, 1.4))):
        drum = kit.cyl("Drum%s" % k, PIER_R, 0.7, (x, y, PIER_R), m.stone, verts=16, rot=(math.pi / 2, 0, yaw))
        kit.cyl_uv(drum, 1.5)
    for k in range(6):
        s = 0.12 + rng.random() * 0.14
        kit.box("Chip%s" % k, (s * 1.3, s, s * 0.7), (-0.6 + rng.random() * 1.8, -0.9 + rng.random() * 1.6, s * 0.3), m.stone, rot=(rng.random(), rng.random(), rng.random() * 3))
    for o in kit.meshes():
        lab.uv_box(o, 1.0)
    kit.cyl_uv(shaft, 1.5)
    return finish("broken_pillar")


def build_rubble_pile():
    """A heap of fallen vault stones and a broken rib."""
    kit.reset()
    m = lab.Mats()
    rng = lab.rng_for("rubble_pile")
    for k in range(22):
        a = rng.random() * math.tau
        d = rng.random() * 0.9
        s = 0.18 + rng.random() * 0.28
        z = s * 0.35 + (0.9 - d) * 0.45
        kit.box("Stone%s" % k, (s * 1.5, s, s * 0.75), (d * math.cos(a) * 1.3, d * math.sin(a), z), m.stone if k % 3 else m.stone_trim, rot=(rng.random() * 0.6, rng.random() * 0.6, rng.random() * 3))
    kit.box("Rib", (2.2, 0.36, 0.3), (0.2, 0.3, 0.75), m.stone_trim, rot=(0.0, 0.32, 0.5))
    for o in kit.meshes():
        lab.uv_box(o, 1.0)
    return finish("rubble_pile")


BUILDERS = {
    "great_crypt_hall": build_hall,
    "summoning_circle": build_summoning_circle,
    "broken_pillar": build_broken_pillar,
    "rubble_pile": build_rubble_pile,
}


def write_provenance(built):
    folder = out_dir_and_names()[0]
    lines = [
        "# Great crypt models",
        "",
        "Mode: **Reference**. These models are original solids built from",
        "primitives, a larger sibling of the crypt lab in",
        "`assets/verse/generated/chamber/`. A private 1.12.1 Scholomance view",
        "was studied for the kinds of spaces a cult's crypt holds: a vaulted",
        "nave, aisles behind an arcade, side chapels, and a raised dais. No",
        "mesh, texture, font, or UI file from that view is in this folder, and",
        "the models are not copies of those silhouettes.",
        "",
        "Script: `scripts/blender/great_crypt.py`, which reuses the materials",
        "and helpers of `scripts/blender/chamber_lab.py`.",
        "",
        "Command:",
        "",
        "```sh",
        "Blender -b --factory-startup --python scripts/blender/great_crypt.py -- \\",
        "    assets/verse/generated/great_crypt",
        "```",
        "",
        "Blender %s." % built[0]["blender"],
        "",
        "## Textures",
        "",
        "The same images as the crypt lab: `T_Brick_BaseColor` and",
        "`T_RockTrim_BaseColor` from the admitted CC0 1.0 Quaternius Medieval",
        "Village MegaKit (Credit: Quaternius, `assets/verse/everglade/village/`),",
        "downscaled to 256 pixels and tinted, and `T_Lab_Flag`, `T_Lab_Wood`,",
        "and `T_Lab_Iron`, which `chamber_lab.py` authors with NumPy from",
        "seeded value noise. Each model packs its images into its glb as PNG.",
        "",
        "## Props",
        "",
        "The crypt's furniture is the crypt lab's 24 props, reused and",
        "multiplied by `verse_world::great_crypt::LAYOUT`.",
        "",
        "## Collision",
        "",
        "`great_crypt_hall.footprint.json` lists explicit boxes rather than",
        "each part's bounds, so arches and chapel openings stay open.",
        "",
        "| Model | Triangles |",
        "| --- | --- |",
    ]
    for info in built:
        lines.append("| `%s` | %s |" % (os.path.basename(info["out"]), info["triangles"]))
    lines.append("")
    with open(os.path.join(folder, "PROVENANCE.md"), "w") as handle:
        handle.write("\n".join(lines))


def main():
    folder, names = out_dir_and_names()
    os.makedirs(folder, exist_ok=True)
    unknown = [name for name in names if name not in BUILDERS]
    if unknown:
        sys.exit("Unknown model %s" % ", ".join(unknown))
    built = [BUILDERS[name]() for name in names]
    if names == NAMES:
        write_provenance(built)


if __name__ == "__main__":
    main()
