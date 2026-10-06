"""Build the Greco-futurism kit and the owner's house for Verse.

Run headless:
    Blender -b --factory-startup --python scripts/blender/greco_futurism.py -- \
        [OUT_DIR] [NAME ...] [--kit KIT_DIR]

Reference mode: every piece is original geometry built from boxes, prisms,
and flat strips. Only two images are sampled, both from the Medieval
Village MegaKit already admitted into Everglade: the plaster image,
recolored as limestone, and the wood trim image, darkened to walnut.
`docs/verse/greco-futurism.md` is the style guide; read it before changing
a proportion or a color.

Style rules, read from the owner's four reference images:

- A temple front reduced to its essentials: smooth, unfluted columns with
  plain square capitals, a deep flat entablature (architrave, a frieze of
  small inset square panels, and a cornice of three stepped bands), a flat
  roof behind a plain attic, and plain chimney blocks.
- A broad stair of many shallow steps between low planter walls with
  clipped hedges, rising to a portico on a podium.
- The futurism lives in the surfaces and doors: tall bronze doors inlaid
  with rectilinear copper traces in symmetric, stepped "machine glyphs",
  with small glowing amber panes; dark walnut lattice screens; and inside,
  a coffered ceiling, white marble pilasters, and a dark walnut wall
  inscribed with faint circuit lines.
- Palette: warm cream and limestone, dark walnut and bronze, deep
  red-brown, copper, and amber light. The amber panes and the lamp shade
  are named `Emit...`, which Everglade draws glowing (`scene::emits`).

Frame: 1 unit = 1 m; fronts face -Y in Blender, which is +Z after export.
A kit piece's origin is on the ground (or its mounting face) at its front
center. The house's origin is on the ground at the center of the front
edge of its lowest step. OUT_DIR defaults to `assets/verse/generated/greco`.

Models:

- `greco_house`: the owner's house, a two-storey estate on a podium with a
  columned portico, the stepped stair and forecourt between hedged planter
  walls, the bronze circuit door (open, its leaves swung in), lattice
  screen walls, and an enterable great room: a coffered ceiling, marble
  pilasters, a dark walnut wall with an engraved double door and
  bookshelves, a desk, a long sofa, rugs, a lamp, and a planter. It also
  writes `far/greco_house.glb`, the far level of detail.
- Kit pieces under `kit/`, for review and later buildings: `column`,
  `pier`, `entablature_bay`, `stair_flight`, `planter_wall`,
  `circuit_door`, `lattice_screen`, `coffer_bay`, `pilaster`, `chimney`,
  `circuit_panel`, `bench_long`, `planter`, `lamp`, `rug`, `desk`, `sofa`,
  and `bookshelf`.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(__file__))
import buildings as bl  # noqa: E402

# Whether the far level of detail is being built.
FAR = False

# sRGB colors; `docs/verse/greco-futurism.md` lists their linear values.
LIMESTONE = (0.88, 0.82, 0.70)
STONE_SHADE = (0.76, 0.70, 0.59)
MARBLE = (0.91, 0.90, 0.87)
BRONZE = (0.24, 0.15, 0.09)
COPPER = (0.66, 0.38, 0.20)
RED_BROWN = (0.40, 0.18, 0.11)
AMBER = (1.00, 0.68, 0.30)
GLASS = (0.10, 0.11, 0.12)
HEDGE = (0.17, 0.29, 0.12)
LINEN = (0.86, 0.81, 0.71)
RUG_FIELD = (0.76, 0.58, 0.45)

# Heights above the ground, m.
FORECOURT = 0.6  # The forecourt terrace, four steps up.
FLOOR = 1.6  # The podium: the portico's and the great room's floor.
CEILING = FLOOR + 4.6  # The great room's ceiling.
WALL_TOP = CEILING + 3.6  # The top of the walls and the columns.
ENTABLATURE = 1.8  # Architrave, frieze, and cornice, 0.6 m each.
ATTIC = 0.8

# Plan, m (Blender y grows away from the street).
HALF_W = 10.0  # The main block's half width.
FRONT_WALL = 11.6  # The facade's outer face.
BACK_WALL = 25.6
WALL = 0.4
COLUMN_Y = 8.6  # The portico's line of supports.
COLUMN_D = 0.9  # Column diameter: 9 diameters to the 8.2 m shaft.


def materials(b):
    """The style's materials: at most 16, two of them textured."""
    tex = os.path.join(bl.KIT, "glTF")
    px = bl._pixels(os.path.join(tex, "T_Plaster_BaseColor.png"), 512)
    lime = bl._image("T_Plaster_limestone", bl._recolor(px, LIMESTONE))
    px = bl._pixels(os.path.join(tex, "T_WoodTrim_BaseColor.png"), 1024)
    walnut = bl._image("T_WoodTrim_walnut", bl._darken(px, 0.42))
    m = b.mats
    m["lime"] = bl._material("GrecoLimestone", lime, rough=0.8)
    m["shade"] = bl._material("GrecoStoneShade", color=STONE_SHADE, rough=0.85)
    m["marble"] = bl._material("GrecoMarble", color=MARBLE, rough=0.4)
    m["walnut"] = bl._material("GrecoWalnut", walnut, rough=0.6)
    m["bronze"] = bl._material("GrecoBronze", color=BRONZE, rough=0.45)
    m["copper"] = bl._material("GrecoCopper", color=COPPER, rough=0.4)
    m["redbrown"] = bl._material("GrecoRedBrown", color=RED_BROWN, rough=0.8)
    m["amber"] = bl._material("EmitAmber", color=AMBER, rough=0.3, emit=AMBER)
    m["glass"] = bl._material("GrecoGlass", color=GLASS, rough=0.15)
    m["hedge"] = bl._material("GrecoHedge", color=HEDGE, rough=0.95)
    m["linen"] = bl._material("GrecoLinen", color=LINEN, rough=0.95)
    m["rug"] = bl._material("GrecoRug", color=RUG_FIELD, rough=1.0)


WALNUT_BAND = bl.DARK_WOOD
# The outward face of a frieze panel, from the face its back hides.
OUTWARD = {"+y": "-y", "-y": "+y", "+x": "-x", "-x": "+x"}

# --------------------------------------------------------------------------
# Geometry


AXES = {"-x": (0, -1), "+x": (0, 1), "-y": (1, -1), "+y": (1, 1), "-z": (2, -1), "+z": (2, 1)}


def only(face):
    """The `skip` list that keeps one face of a box: a flat inlay."""
    return " ".join(a for a in AXES if a != face)


def box(b, lo, hi, mat, skip="", xf=None, name="Part", smooth=False):
    """A box from `lo` to `hi`, without the faces named in `skip` (such as
    "-z +y"): the faces a wall, the ground, or a neighbor hides. `xf` moves
    the finished box, for boxes built in a local frame."""
    band = WALNUT_BAND if mat == "walnut" else None
    mesh = bl.box_mesh(name, lo, hi, b.mats[mat], band=band, tile=2.0)
    drop = [AXES[s] for s in skip.split()]
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bm.normal_update()
    gone = [f for f in bm.faces if any(f.normal[a] * sign > 0.9 for a, sign in drop)]
    if gone:
        bmesh.ops.delete(bm, geom=gone, context="FACES")
    if xf is not None:
        bmesh.ops.transform(bm, matrix=xf, verts=bm.verts)
    bm.normal_update()
    bm.to_mesh(mesh)
    bm.free()
    return b.solid(name, mesh)


def prism(b, center, radius, z0, z1, sides, mat, name="Shaft", cap=False, rot=None):
    """An upright smooth-shaded prism, a column shaft at enough sides."""
    rot = math.pi / sides if rot is None else rot
    if cap:
        mesh = bl.frustum_mesh(name, center, radius, radius, z0, z1, sides, b.mats[mat], rot=rot)
    else:
        mesh = bl.prism_mesh(name, center, radius, z0, z1, sides, b.mats[mat], rot=rot)
    for p in mesh.polygons:
        p.use_smooth = abs(p.normal.z) < 0.5
    return b.solid(name, mesh)


def slab(b, x0, x1, y0, y1, z, mat, down=False, name="Slab"):
    """One face of a box at height z, facing up (or down), UV tiled."""
    if down:
        return box(b, (x0, y0, z), (x1, y1, z + 0.01), mat, skip="+z -x +x -y +y", name=name)
    return box(b, (x0, y0, z - 0.01), (x1, y1, z), mat, skip="-z -x +x -y +y", name=name)


def sphere(b, center, r, mat, segments=8, rings=5, squash=0.8, name="Ball"):
    """A low, smooth-shaded ball, flattened a little: a clipped shrub."""
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=segments, v_segments=rings, radius=r)
    bmesh.ops.scale(bm, vec=Vector((1.0, 1.0, squash)), verts=bm.verts)
    bmesh.ops.translate(bm, vec=Vector(center), verts=bm.verts)
    uv = bm.loops.layers.uv.new("UVMap")
    for f in bm.faces:
        for loop in f.loops:
            loop[uv].uv = (loop.vert.co.x / 2.0, loop.vert.co.z / 2.0)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    for p in mesh.polygons:
        p.use_smooth = True
    mesh.materials.append(b.mats[mat])
    return b.solid(name, mesh)


def local(origin, rot=0.0):
    """A wall's local frame: x along it, -y out of it, z up."""
    return Matrix.Translation(Vector(origin)) @ Matrix.Rotation(math.radians(rot), 4, "Z")


# --------------------------------------------------------------------------
# The circuit glyph: one symmetric pattern for doors, transoms, and panels


def glyph_segments(tall=True):
    """The machine glyph's traces in normalized door coordinates: s across
    from -0.5 to 0.5, t up from 0 to 1. Each is a polyline on the left
    half, mirrored onto the right; pads are small squares at trace ends;
    panes are the amber lights. Rectilinear, stepped, and symmetric."""
    lines = [
        # The outer border and the inner frame with its stepped head.
        [(-0.44, 0.04), (-0.44, 0.96), (0.0, 0.96)],
        [(-0.34, 0.10), (-0.34, 0.84), (-0.26, 0.84), (-0.26, 0.90), (-0.14, 0.90), (-0.14, 0.93), (0.0, 0.93)],
        [(-0.34, 0.10), (0.0, 0.10)],
        # The frame round the panes, and the steps falling from it.
        [(-0.16, 0.40), (-0.16, 0.66), (0.0, 0.66)],
        [(-0.16, 0.40), (0.0, 0.40)],
        [(-0.16, 0.46), (-0.26, 0.46), (-0.26, 0.34), (-0.34, 0.34)],
        [(-0.16, 0.60), (-0.24, 0.60), (-0.24, 0.72), (-0.34, 0.72)],
        # The spine and its branches, with pads at their ends.
        [(-0.05, 0.10), (-0.05, 0.30), (-0.20, 0.30), (-0.20, 0.22)],
        [(-0.05, 0.66), (-0.05, 0.80), (-0.18, 0.80)],
        [(-0.10, 0.16), (-0.28, 0.16)],
    ]
    pads = [(-0.20, 0.22), (-0.18, 0.80), (-0.28, 0.16), (-0.26, 0.90)]
    panes = [(-0.12, 0.43, -0.02, 0.52), (-0.12, 0.54, -0.02, 0.63)]
    if not tall:
        # A transom's band: frame, spine steps, and a row of slits.
        lines = [
            [(-0.46, 0.10), (-0.46, 0.90), (0.0, 0.90)],
            [(-0.46, 0.10), (0.0, 0.10)],
            [(-0.36, 0.25), (-0.36, 0.75), (-0.22, 0.75), (-0.22, 0.25)],
            [(-0.12, 0.10), (-0.12, 0.40), (-0.04, 0.40)],
        ]
        pads = [(-0.29, 0.5)]
        panes = [(-0.10, 0.52, -0.02, 0.78)]
    return lines, pads, panes


def glyph(b, xf, w, h, proud=0.02, trace=0.035, tall=True, base="bronze", leaf=True):
    """A circuit relief on a panel `w` wide and `h` tall whose face lies in
    the local plane y = 0, facing -y, from u = -w/2 to w/2 and z = 0 to h.
    With `leaf`, the panel itself is drawn too (a door leaf or a plate)."""
    if leaf:
        box(b, (-w / 2, 0.0, 0.0), (w / 2, 0.06, h), base, skip="+y", xf=xf, name="Leaf")
    if FAR:
        return
    lines, pads, panes = glyph_segments(tall)
    tw = trace / 2

    def strip(p, q):
        (s0, t0), (s1, t1) = p, q
        u0, u1 = sorted((s0 * w, s1 * w))
        z0, z1 = sorted((t0 * h, t1 * h))
        # A flat copper inlay: only its face shows at 2 cm proud.
        box(b, (u0 - tw, -proud, z0 - tw), (u1 + tw, 0.0, z1 + tw), "copper", skip=only("-y"), xf=xf,
            name="Trace")

    for line in lines:
        for mirror in (1, -1):
            pts = [(s * mirror, t) for s, t in line]
            for p, q in zip(pts, pts[1:]):
                if abs(p[0] - q[0]) < 1e-6 and abs(p[1] - q[1]) < 1e-6:
                    continue
                strip(p, q)
    for s, t in pads:
        for mirror in (1, -1):
            u, z = s * mirror * w, t * h
            box(b, (u - 0.05, -proud - 0.01, z - 0.05), (u + 0.05, 0.0, z + 0.05), "copper", skip=only("-y"),
                xf=xf, name="Pad")
    for s0, t0, s1, t1 in panes:
        for mirror in (1, -1):
            u0, u1 = sorted((s0 * mirror * w, s1 * mirror * w))
            box(b, (u0, -proud * 0.5, t0 * h), (u1, 0.0, t1 * h), "amber", skip="+y", xf=xf, name="Pane")


# --------------------------------------------------------------------------
# Kit pieces. Each adds its geometry to builder `b`.


def column(b, x, y, z0, z1, d=COLUMN_D, sides=12):
    """A smooth, unfluted column: a square plinth, a torus-like base ring,
    the shaft, a necking ring, and a plain square capital."""
    r = d / 2
    box(b, (x - r - 0.12, y - r - 0.12, z0), (x + r + 0.12, y + r + 0.12, z0 + 0.22), "shade", skip="-z",
        name="Plinth")
    if not FAR:
        prism(b, (x, y), r + 0.06, z0 + 0.22, z0 + 0.36, sides, "lime", name="BaseRing", cap=True)
    prism(b, (x, y), r, z0 + 0.22, z1 - 0.5, sides if not FAR else 6, "lime")
    if not FAR:
        prism(b, (x, y), r + 0.05, z1 - 0.62, z1 - 0.5, sides, "lime", name="Necking")
    box(b, (x - r - 0.08, y - r - 0.08, z1 - 0.5), (x + r + 0.08, y + r + 0.08, z1 - 0.2), "lime", name="Echinus",
        skip="-z" if FAR else "")
    box(b, (x - r - 0.18, y - r - 0.18, z1 - 0.2), (x + r + 0.18, y + r + 0.18, z1), "lime", skip="+z",
        name="Abacus")
    b.collide("column", (x - r - 0.12, y - r - 0.12, z0), (x + r + 0.12, y + r + 0.12, z1))


def pier(b, x, y, z0, z1, d=COLUMN_D):
    """A square pier (an anta) with the column's plinth and capital."""
    r = d / 2
    box(b, (x - r - 0.1, y - r - 0.1, z0), (x + r + 0.1, y + r + 0.1, z0 + 0.3), "shade", skip="-z",
        name="Plinth")
    box(b, (x - r, y - r, z0 + 0.3), (x + r, y + r, z1 - 0.3), "lime", skip="-z +z", name="Pier")
    box(b, (x - r - 0.12, y - r - 0.12, z1 - 0.3), (x + r + 0.12, y + r + 0.12, z1), "lime", skip="+z",
        name="PierCap")
    b.collide("pier", (x - r - 0.1, y - r - 0.1, z0), (x + r + 0.1, y + r + 0.1, z1))


def entablature(b, x0, x1, y0, y1, z, panels=("-y", "-x", "+x", "+y"), spacing=1.6):
    """The deep flat entablature over a plan from (x0, y0) to (x1, y1):
    a plain architrave, a frieze of small inset red-brown square panels,
    and a cornice of three stepped bands, each projecting further."""
    a, f = z + 0.6, z + 1.2
    box(b, (x0, y0, z), (x1, y1, a), "lime", skip="+z", name="Architrave")
    box(b, (x0 + 0.04, y0 + 0.04, a), (x1 - 0.04, y1 - 0.04, f), "lime", skip="-z +z", name="Frieze")
    for k, (out, top) in enumerate(((0.12, f + 0.18), (0.3, f + 0.4), (0.55, f + 0.6))):
        bottom = f if k == 0 else f + (0.18, 0.4)[k - 1]
        box(b, (x0 - out, y0 - out, bottom), (x1 + out, y1 + out, top), "shade" if k == 1 else "lime",
            skip="+z" if k < 2 else "", name="Cornice")
    if FAR:
        return
    # The frieze's panels: small squares set into the frieze, a bronze
    # bead round a red-brown field.
    zc = (a + f) / 2
    s = 0.2
    runs = {
        "-y": (x0, x1, lambda u: ((u - s, y0 + 0.04 - 0.03, zc - s), (u + s, y0 + 0.04, zc + s)), "+y"),
        "+y": (x0, x1, lambda u: ((u - s, y1 - 0.04, zc - s), (u + s, y1 - 0.04 + 0.03, zc + s)), "-y"),
        "-x": (y0, y1, lambda u: ((x0 + 0.04 - 0.03, u - s, zc - s), (x0 + 0.04, u + s, zc + s)), "+x"),
        "+x": (y0, y1, lambda u: ((x1 - 0.04, u - s, zc - s), (x1 - 0.04 + 0.03, u + s, zc + s)), "-x"),
    }
    for side in panels:
        lo, hi, at, back = runs[side]
        n = max(1, int((hi - lo - 0.8) // spacing))
        step = (hi - lo) / (n + 1)
        for i in range(n):
            u = lo + step * (i + 1)
            p, q = at(u)
            box(b, p, q, "redbrown", skip=only(OUTWARD[back]), name="FriezePanel")


def stair(b, x0, x1, y0, z0, steps, rise, run, mat="shade"):
    """A flight of shallow steps from the front edge y0 at height z0, each
    step one solid block down to z0 so its front and top show."""
    for i in range(steps):
        y = y0 + i * run
        box(b, (x0, y, z0), (x1, y + run, z0 + rise * (i + 1)), mat, skip="-z +y -x +x", name="Step")
    return y0 + steps * run, z0 + steps * rise


def planter_wall(b, x0, x1, y0, y1, z0, top, hedge=0.8, ends="-x +x"):
    """A low limestone planter wall with a clipped hedge standing in it."""
    box(b, (x0, y0, z0), (x1, y1, top), "lime", skip="-z", name="Planter")
    if not FAR:
        box(b, (x0 - 0.04, y0 - 0.04, top - 0.08), (x1 + 0.04, y1 + 0.04, top), "shade", skip="-z",
            name="Coping")
    if hedge > 0:
        inset = 0.1
        box(b, (x0 + inset, y0 + inset, top), (x1 - inset, y1 - inset, top + hedge), "hedge", skip="-z",
            name="Hedge")
        if not FAR and (x1 - x0) > 1.6 and (y1 - y0) > 1.0:
            # A second, softer tier so the clipped top doesn't read as a box.
            box(b, (x0 + 0.35, y0 + 0.35, top + hedge), (x1 - 0.35, y1 - 0.35, top + hedge + 0.18), "hedge",
                skip="-z", name="Hedge")
    b.collide("planter", (x0, y0, z0), (x1, y1, top + hedge))


def lattice(b, x0, x1, z0, z1, y, depth=0.12, cell=0.2, coarse=4):
    """A walnut lattice screen across an opening: a fine grid of bars with
    heavier bars every `coarse` cells, like a bookcase or a mashrabiya."""
    if FAR:
        box(b, (x0, y - 0.02, z0), (x1, y, z1), "bronze", skip="+y", name="Screen")
        return
    nx = max(2, round((x1 - x0) / cell))
    nz = max(2, round((z1 - z0) / cell))
    for i in range(1, nx):
        x = x0 + (x1 - x0) * i / nx
        t = 0.035 if i % coarse else 0.09
        box(b, (x - t / 2, y, z0), (x + t / 2, y + depth, z1), "walnut", skip="-z +z", name="LatticeBar")
    for k in range(1, nz):
        z = z0 + (z1 - z0) * k / nz
        t = 0.035 if k % coarse else 0.09
        box(b, (x0, y - 0.005, z - t / 2), (x1, y + depth + 0.005, z + t / 2), "walnut", skip="-x +x",
            name="LatticeBar")
    # The frame round the screen.
    for lo, hi in (((x0, y - 0.03, z0), (x0 + 0.12, y + depth + 0.03, z1)),
                   ((x1 - 0.12, y - 0.03, z0), (x1, y + depth + 0.03, z1)),
                   ((x0, y - 0.03, z1 - 0.12), (x1, y + depth + 0.03, z1)),
                   ((x0, y - 0.03, z0), (x1, y + depth + 0.03, z0 + 0.12))):
        box(b, lo, hi, "bronze", name="LatticeFrame", skip="")


def chimney(b, x, y, z0, top, w=1.0, d=1.2):
    """A plain limestone chimney block with a cap band."""
    box(b, (x - w / 2, y - d / 2, z0), (x + w / 2, y + d / 2, top), "lime", skip="-z", name="Chimney")
    box(b, (x - w / 2 - 0.1, y - d / 2 - 0.1, top), (x + w / 2 + 0.1, y + d / 2 + 0.1, top + 0.25), "shade",
        name="ChimneyCap")
    if not FAR:
        box(b, (x - w / 2 + 0.15, y - d / 2 + 0.15, top + 0.25), (x + w / 2 - 0.15, y + d / 2 - 0.15, top + 0.45),
            "shade", skip="-z", name="ChimneyCap")
    b.chimneys.append((round(x, 2), round(top + 0.45, 2), round(-y, 2)))


def pilaster(b, xf, u, z0, z1, w=0.7, out=0.16):
    """A white marble pilaster on a wall's face, with a base and a capital."""
    box(b, (u - w / 2, -out, z0 + 0.25), (u + w / 2, 0.0, z1 - 0.35), "marble", skip="+y -z +z", xf=xf,
        name="Pilaster")
    box(b, (u - w / 2 - 0.06, -out - 0.06, z0), (u + w / 2 + 0.06, 0.0, z0 + 0.25), "marble", skip="+y -z",
        xf=xf, name="PilasterBase")
    box(b, (u - w / 2 - 0.1, -out - 0.1, z1 - 0.35), (u + w / 2 + 0.1, 0.0, z1), "marble", skip="+y +z", xf=xf,
        name="PilasterCap")


def coffers(b, x0, x1, y0, y1, z, xs, ys, depth=0.5, width=0.36):
    """A coffered ceiling under the slab at height z: crossing beams, and a
    stepped cove round the room's edge."""
    for x in xs:
        box(b, (x - width / 2, y0, z - depth), (x + width / 2, y1, z), "lime", skip="+z -y +y", name="Coffer")
    for y in ys:
        box(b, (x0, y - width / 2, z - depth), (x1, y + width / 2, z), "lime", skip="+z -x +x", name="Coffer")
    for out, drop, mat in ((0.3, 0.5, "shade"), (0.55, 0.28, "lime")):
        box(b, (x0, y0, z - drop), (x1, y0 + out, z), mat, skip="+z -x +x -y", name="Cove")
        box(b, (x0, y1 - out, z - drop), (x1, y1, z), mat, skip="+z -x +x +y", name="Cove")
        box(b, (x0, y0, z - drop), (x0 + out, y1, z), mat, skip="+z -y +y -x", name="Cove")
        box(b, (x1 - out, y0, z - drop), (x1, y1, z), mat, skip="+z -y +y +x", name="Cove")


def dentils(b, xf, u0, u1, z, tooth=0.12, gap=0.14, out=0.12, tall=0.16):
    """A row of dentils under a cornice on a wall's face."""
    u = u0
    while u + tooth <= u1:
        box(b, (u, -out, z - tall), (u + tooth, 0.0, z), "marble", skip="+y +z -x +x", xf=xf, name="Dentil")
        u += tooth + gap


def circuit_lines(b, xf, w, z0, h):
    """Faint circuit lines inscribed on a dark walnut wall: thin copper
    strips barely proud of the panel, symmetric about its center."""
    lines = [
        [(-0.48, 0.92), (-0.30, 0.92), (-0.30, 0.78), (-0.22, 0.78)],
        [(-0.45, 0.70), (-0.38, 0.70), (-0.38, 0.55)],
        [(-0.48, 0.85), (-0.42, 0.85), (-0.42, 0.62)],
        [(-0.26, 0.98), (-0.26, 0.86), (-0.16, 0.86)],
    ]
    for line in lines:
        for mirror in (1, -1):
            pts = [(s * mirror * w, z0 + t * h) for s, t in line]
            for (u0, za), (u1, zb) in zip(pts, pts[1:]):
                ua, ub = sorted((u0, u1))
                z_0, z_1 = sorted((za, zb))
                box(b, (ua - 0.015, -0.012, z_0 - 0.015), (ub + 0.015, 0.0, z_1 + 0.015), "copper",
                    skip="+y -x +x -z +z", xf=xf, name="CircuitLine")


def bookshelf(b, xf, u, z0, w=1.8, h=2.5, d=0.42):
    """A dark walnut bookcase against a wall, with rows of books."""
    box(b, (u - w / 2, -d, z0), (u - w / 2 + 0.06, 0.0, z0 + h), "walnut", skip="+y -z", xf=xf, name="Case")
    box(b, (u + w / 2 - 0.06, -d, z0), (u + w / 2, 0.0, z0 + h), "walnut", skip="+y -z", xf=xf, name="Case")
    box(b, (u - w / 2, -d, z0 + h - 0.06), (u + w / 2, 0.0, z0 + h), "walnut", skip="+y", xf=xf, name="Case")
    rows = 5
    for k in range(rows):
        z = z0 + 0.1 + k * (h - 0.16) / rows
        box(b, (u - w / 2 + 0.06, -d, z - 0.04), (u + w / 2 - 0.06, 0.0, z), "walnut", skip="+y -x +x", xf=xf,
            name="Shelf")
        if FAR:
            continue
        # Books: three runs a shelf in the palette's colors.
        x = u - w / 2 + 0.1
        for j, (mat, run, tall) in enumerate((("linen", 0.5, 0.3), ("redbrown", 0.55, 0.34), ("walnut", 0.45, 0.28))):
            shift = ((k + j) % 3) * 0.05
            box(b, (x, -d + 0.06, z), (x + run - 0.05, -0.02, z + tall - shift), mat, skip="+y -z", xf=xf,
                name="Books")
            x += run


def desk(b, x, y, z0, rot=0.0):
    """A long walnut desk on two pedestals and bronze sled legs, with a
    copper edge, and a chair behind it."""
    xf = local((x, y, z0), rot)
    w, d, h = 2.8, 1.0, 0.76
    box(b, (-w / 2, -d / 2, h - 0.06), (w / 2, d / 2, h), "walnut", xf=xf, name="DeskTop")
    box(b, (-w / 2 - 0.02, -d / 2 - 0.02, h - 0.09), (w / 2 + 0.02, d / 2 + 0.02, h - 0.06), "copper", skip="+z",
        xf=xf, name="DeskEdge")
    for s in (-1, 1):
        box(b, (s * 0.95 - 0.28, -d / 2 + 0.08, 0.12), (s * 0.95 + 0.28, d / 2 - 0.08, h - 0.09), "walnut",
            skip="+z", xf=xf, name="Pedestal")
        box(b, (s * 1.3 - 0.03, -d / 2 + 0.05, 0.0), (s * 1.3 + 0.03, d / 2 - 0.05, 0.04), "copper", skip="-z",
            xf=xf, name="Sled")
        box(b, (s * 1.3 - 0.03, -0.03, 0.04), (s * 1.3 + 0.03, 0.03, h - 0.09), "copper", xf=xf, name="Sled")
    if FAR:
        return
    # A chair behind the desk, facing it.
    box(b, (-0.28, 0.75, 0.42), (0.28, 1.25, 0.5), "redbrown", xf=xf, name="Chair")
    box(b, (-0.28, 1.2, 0.5), (0.28, 1.28, 1.0), "redbrown", xf=xf, name="Chair")
    box(b, (-0.03, 0.97, 0.0), (0.03, 1.03, 0.42), "copper", xf=xf, name="ChairPost")
    box(b, (-0.3, 0.8, 0.0), (0.3, 1.2, 0.03), "copper", skip="-z", xf=xf, name="ChairFoot")
    # A slim lamp and a bowl on the desk.
    box(b, (-1.2, -0.1, h), (-1.16, -0.06, h + 0.45), "copper", xf=xf, name="DeskLamp")
    box(b, (-1.22, -0.12, h + 0.45), (-0.95, -0.04, h + 0.5), "amber", xf=xf, name="DeskLamp")
    box(b, (0.6, -0.2, h), (1.0, 0.15, h + 0.08), "bronze", skip="-z", xf=xf, name="Bowl")
    b.collide("desk", Vector(xf @ Vector((-w / 2, -d / 2, 0))), Vector(xf @ Vector((w / 2, d / 2, h))))


def sofa(b, x, y, z0, rot=0.0, w=4.4):
    """A long low linen sofa on a dark bronze plinth, with red-brown
    cushions, as in the references' great room."""
    xf = local((x, y, z0), rot)
    d = 1.0
    box(b, (-w / 2, -d / 2, 0.0), (w / 2, d / 2, 0.12), "bronze", skip="-z", xf=xf, name="SofaPlinth")
    box(b, (-w / 2 + 0.25, -d / 2 + 0.02, 0.12), (w / 2 - 0.25, d / 2 - 0.25, 0.44), "linen", xf=xf,
        skip="-z", name="Seat")
    box(b, (-w / 2, d / 2 - 0.28, 0.12), (w / 2, d / 2, 0.82), "linen", skip="-z", xf=xf, name="Back")
    for s in (-1, 1):
        box(b, (s * w / 2 - 0.25 if s > 0 else -w / 2, -d / 2, 0.12),
            (w / 2 if s > 0 else -w / 2 + 0.25, d / 2 - 0.28, 0.6), "linen", skip="-z", xf=xf, name="Arm")
    if not FAR:
        box(b, (-0.02, -d / 2 + 0.02, 0.44), (0.02, d / 2 - 0.25, 0.46), "linen", xf=xf, name="SeatSeam")
        for u in (-w / 2 + 0.6, w / 2 - 0.7):
            box(b, (u - 0.25, d / 2 - 0.45, 0.44), (u + 0.25, d / 2 - 0.28, 0.86), "redbrown", skip="-z", xf=xf,
                name="Cushion")
    b.collide("sofa", Vector(xf @ Vector((-w / 2, -d / 2, 0))), Vector(xf @ Vector((w / 2, d / 2, 0.82))))


def low_table(b, x, y, z0, rot=0.0, w=2.4, d=0.9):
    """A long low walnut table with an open shelf under it."""
    xf = local((x, y, z0), rot)
    box(b, (-w / 2, -d / 2, 0.36), (w / 2, d / 2, 0.42), "walnut", xf=xf, name="TableTop")
    box(b, (-w / 2 + 0.12, -d / 2 + 0.12, 0.1), (w / 2 - 0.12, d / 2 - 0.12, 0.14), "walnut", xf=xf,
        name="TableShelf")
    for s in (-1, 1):
        box(b, (s * w / 2 - (0.1 if s > 0 else 0.0), -d / 2, 0.0), (s * w / 2 + (0.0 if s > 0 else 0.1), d / 2, 0.36),
            "bronze", skip="-z +z", xf=xf, name="TableEnd")
    if not FAR:
        box(b, (-0.5, -0.2, 0.42), (-0.1, 0.1, 0.47), "linen", skip="-z", xf=xf, name="Book")


def bench_long(b, x, y, z0, rot=0.0):
    """The forecourt's long low walnut table with bronze legs, and a stool
    at each end, as in the references' entrance court."""
    xf = local((x, y, z0), rot)
    w, d, h = 2.6, 0.9, 0.62
    box(b, (-w / 2, -d / 2, h - 0.12), (w / 2, d / 2, h), "walnut", xf=xf, name="Bench")
    box(b, (-w / 2 + 0.1, -d / 2 + 0.05, h - 0.42), (w / 2 - 0.1, d / 2 - 0.05, h - 0.12), "walnut", xf=xf,
        skip="+z", name="BenchApron")
    for s in (-1, 1):
        for t in (-1, 1):
            box(b, (s * (w / 2 - 0.12) - 0.04, t * (d / 2 - 0.08) - 0.04, 0.0),
                (s * (w / 2 - 0.12) + 0.04, t * (d / 2 - 0.08) + 0.04, h - 0.42), "bronze", skip="-z +z", xf=xf,
                name="BenchLeg")
    if not FAR:
        for s in (-1, 1):
            u = s * (w / 2 + 0.7)
            box(b, (u - 0.3, -0.18, 0.4), (u + 0.3, 0.18, 0.46), "walnut", xf=xf, name="Stool")
            box(b, (u - 0.25, -0.03, 0.0), (u + 0.25, 0.03, 0.4), "bronze", skip="-z +z", xf=xf, name="StoolLeg")


def planter(b, x, y, z0, size=0.9, ball=1.0):
    """A square limestone planter with a clipped shrub."""
    h = 0.7
    box(b, (x - size / 2, y - size / 2, z0), (x + size / 2, y + size / 2, z0 + h), "lime", skip="-z",
        name="Planter")
    box(b, (x - size / 2 - 0.05, y - size / 2 - 0.05, z0 + h - 0.06), (x + size / 2 + 0.05, y + size / 2 + 0.05,
                                                                        z0 + h), "shade", name="Coping")
    r = ball / 2
    if FAR:
        box(b, (x - r * 0.8, y - r * 0.8, z0 + h), (x + r * 0.8, y + r * 0.8, z0 + h + r * 1.4), "hedge",
            skip="-z", name="Shrub")
    else:
        sphere(b, (x, y, z0 + h + r * 0.75), r, "hedge", segments=7, rings=4, name="Shrub")


def lamp(b, x, y, z0, height=1.7):
    """A bronze floor lamp with a glowing amber drum shade."""
    box(b, (x - 0.2, y - 0.2, z0), (x + 0.2, y + 0.2, z0 + 0.04), "bronze", skip="-z", name="LampFoot")
    box(b, (x - 0.025, y - 0.025, z0 + 0.04), (x + 0.025, y + 0.025, z0 + height - 0.3), "copper", skip="-z +z",
        name="LampPole")
    prism(b, (x, y), 0.22, z0 + height - 0.3, z0 + height, 8, "amber", name="EmitShade", cap=True)


def rug(b, x0, x1, y0, y1, z, border=0.35):
    """A thin rug: a sand field inside a red-brown classical border."""
    t = 0.02
    box(b, (x0 + border, y0 + border, z), (x1 - border, y1 - border, z + t), "rug", skip="-z -x +x -y +y",
        name="RugField")
    for lo, hi in (((x0, y0, z), (x1, y0 + border, z + t)), ((x0, y1 - border, z), (x1, y1, z + t)),
                   ((x0, y0 + border, z), (x0 + border, y1 - border, z + t)),
                   ((x1 - border, y0 + border, z), (x1, y1 - border, z + t))):
        box(b, lo, hi, "redbrown", skip="-z", name="RugBorder")
    if not FAR:
        # The border's inner fillet, in walnut.
        f = 0.06
        x0b, x1b, y0b, y1b = x0 + border + f, x1 - border - f, y0 + border + f, y1 - border - f
        for lo, hi in (((x0b, y0b, z), (x1b, y0b + f, z + t + 0.002)), ((x0b, y1b - f, z), (x1b, y1b, z + t + 0.002)),
                       ((x0b, y0b + f, z), (x0b + f, y1b - f, z + t + 0.002)),
                       ((x1b - f, y0b + f, z), (x1b, y1b - f, z + t + 0.002))):
            box(b, lo, hi, "walnut", skip="-z -x +x -y +y", name="RugFillet")


# The 5 by 7 letters Verse draws its scene labels with
# (`verse_core::label`), for the letters the inscription uses.
LETTERS = {
    "E": [31, 16, 16, 30, 16, 16, 31],
    "F": [31, 16, 16, 30, 16, 16, 16],
    "H": [17, 17, 17, 31, 17, 17, 17],
    "N": [17, 25, 21, 19, 17, 17, 17],
    "O": [14, 17, 17, 17, 17, 17, 14],
    "R": [30, 17, 17, 30, 20, 18, 17],
    "S": [15, 16, 16, 14, 1, 1, 30],
    "T": [31, 4, 4, 4, 4, 4, 4],
    "U": [17, 17, 17, 17, 17, 17, 14],
    "W": [17, 17, 17, 21, 21, 21, 10],
}


def inscription(b, text, y, z, height=0.36, proud=0.03):
    """Bronze letters cut into an architrave's face at y, facing -y, their
    bottom at z, centered on x = 0: each run of lit cells in a row is one
    flat strip just proud of the face."""
    cell = height / 7.0
    width = len(text) * 6 * cell - cell
    for i, ch in enumerate(text):
        rows = LETTERS.get(ch)
        if rows is None:
            continue
        for r, bits in enumerate(rows):
            zr = z + (6 - r) * cell
            col = 0
            while col < 5:
                if bits & (1 << (4 - col)):
                    start = col
                    while col < 5 and bits & (1 << (4 - col)):
                        col += 1
                    x0 = -width / 2 + (i * 6 + start) * cell
                    x1 = -width / 2 + (i * 6 + col) * cell
                    box(b, (x0, y - proud, zr), (x1, y, zr + cell), "bronze", skip="+y -x +x -z +z",
                        name="Letter")
                else:
                    col += 1


# --------------------------------------------------------------------------
# The owner's house


def house_body(b):
    hw = HALF_W
    fy, by = FRONT_WALL, BACK_WALL
    # -- The approach: the lower flight, the forecourt, the upper flight.
    lower_x = 4.5
    y_forecourt, _ = stair(b, -lower_x, lower_x, 0.0, 0.0, 4, FORECOURT / 4, 0.42)
    upper_y = 5.6
    y_portico = upper_y + 6 * 0.38
    box(b, (-hw - 0.4, y_forecourt, 0.0), (hw + 0.4, y_portico, FORECOURT), "shade", skip="-z",
        name="Forecourt")
    stair(b, -4.4, 4.4, upper_y, FORECOURT, 6, (FLOOR - FORECOURT) / 6, 0.38)
    # Hedge banks along the street on each side of the lower flight.
    for s in (-1, 1):
        x0, x1 = sorted((s * lower_x, s * (hw + 0.4)))
        planter_wall(b, x0, x1, 0.0, y_forecourt, 0.0, 0.45, hedge=0.75)
    # The cheek planters beside the upper flight.
    for s in (-1, 1):
        x0, x1 = sorted((s * 4.4, s * 7.2))
        planter_wall(b, x0, x1, upper_y, y_portico, FORECOURT, FLOOR + 0.5, hedge=0.6)
    # The podium under the portico and the house, with a darker base course.
    box(b, (-hw - 0.4, y_portico, 0.0), (hw + 0.4, fy, FLOOR), "lime", skip="-z", name="Podium")
    box(b, (-hw - 0.4, fy, 0.0), (hw + 0.4, by + 0.4, FLOOR), "lime", skip="-z +z", name="Podium")
    box(b, (-hw - 0.48, y_portico + 0.6, 0.0), (hw + 0.48, by + 0.48, 0.3), "shade", skip="-z",
        name="BaseCourse")
    # The podium's top outside the walls, and the great room's marble floor.
    for x0, x1 in ((-hw - 0.4, -hw), (hw, hw + 0.4)):
        slab(b, x0, x1, fy, by + 0.4, FLOOR, "lime", name="PodiumTop")
    slab(b, -hw, hw, by, by + 0.4, FLOOR, "lime", name="PodiumTop")
    slab(b, -hw + WALL, hw - WALL, fy + WALL, by - WALL, FLOOR, "marble", name="Floor")
    # -- The portico: square piers at the ends, round columns at the door.
    for x in (-9.4, -6.6, 6.6, 9.4):
        pier(b, x, COLUMN_Y, FLOOR, WALL_TOP)
    for x in (-2.1, 2.1):
        column(b, x, COLUMN_Y, FLOOR, WALL_TOP)
    # The portico's ceiling beams, between the supports and the facade.
    if not FAR:
        for x in (-8.0, -4.35, 0.0, 4.35, 8.0):
            box(b, (x - 0.18, COLUMN_Y, WALL_TOP - 0.3), (x + 0.18, fy, WALL_TOP), "lime", skip="+z -y +y",
                name="PorticoBeam")
    # -- The facade: piers of wall round the door, two lattice screens, and
    # a band of dark windows above.
    door_hw, screen = 1.25, (2.6, 6.0)
    door_top, transom = FLOOR + 4.0, CEILING
    t = WALL
    for x0, x1 in ((-hw, -screen[1]), (screen[1], hw)):
        box(b, (x0, fy, FLOOR), (x1, fy + t, WALL_TOP), "lime", skip="-z +z", name="Wall")
    for x0, x1 in ((-screen[0], -door_hw), (door_hw, screen[0])):
        box(b, (x0, fy, FLOOR), (x1, fy + t, CEILING), "lime", skip="-z +z", name="Wall")
    for s in (-1, 1):
        x0, x1 = sorted((s * screen[0], s * screen[1]))
        box(b, (x0, fy, door_top), (x1, fy + t, CEILING), "lime", skip="+z -x +x", name="Wall")
        lattice(b, x0, x1, FLOOR, door_top, fy + 0.14)
    box(b, (-screen[1], fy, CEILING), (screen[1], fy + t, WALL_TOP), "lime", skip="-x +x +z", name="Wall")
    # The string course at the ceiling's height, across the facade.
    box(b, (-hw, fy - 0.12, CEILING - 0.05), (hw, fy, CEILING + 0.25), "shade", skip="+y", name="StringCourse")
    # The upper windows: dark glass in bronze frames between mullions.
    if FAR:
        box(b, (-5.6, fy - 0.03, CEILING + 0.6), (5.6, fy, WALL_TOP - 0.5), "glass", skip="+y", name="Glass")
    else:
        box(b, (-5.6, fy - 0.03, CEILING + 0.6), (5.6, fy, WALL_TOP - 0.5), "glass", skip="+y -z +z -x +x",
            name="Glass")
        for k in range(6):
            x = -5.6 + 11.2 * k / 5
            box(b, (x - 0.07, fy - 0.1, CEILING + 0.6), (x + 0.07, fy, WALL_TOP - 0.5), "bronze", skip="+y",
                name="Mullion")
        for z in (CEILING + 0.6, WALL_TOP - 0.5):
            box(b, (-5.67, fy - 0.1, z - 0.07), (5.67, fy, z + 0.07), "bronze", skip="+y", name="Mullion")
    # The door: a bronze surround, the transom's glyph, and two tall leaves
    # swung in against the reveals.
    box(b, (-door_hw - 0.22, fy - 0.08, FLOOR), (-door_hw, fy + t, door_top), "bronze", skip="-z +y",
        name="DoorFrame")
    box(b, (door_hw, fy - 0.08, FLOOR), (door_hw + 0.22, fy + t, door_top), "bronze", skip="-z +y",
        name="DoorFrame")
    box(b, (-door_hw - 0.22, fy - 0.08, door_top), (door_hw + 0.22, fy + t, transom), "bronze", skip="+y +z",
        name="Transom")
    glyph(b, local((0, fy - 0.08, door_top), 0.0), 2.5, transom - door_top, tall=False, leaf=False)
    leaf_w = door_hw - 0.02
    for s in (-1, 1):
        # Hinged at the inner face of the jamb, opened 90 degrees inward;
        # the engraved face, which faced the street, faces the doorway.
        hinge = Vector((s * door_hw, fy + t, FLOOR))
        xf = (Matrix.Translation(hinge) @ Matrix.Rotation(math.radians(-90 * s), 4, "Z")
              @ Matrix.Translation(Vector((-s * leaf_w / 2, -0.06, 0.0))))
        glyph(b, xf, leaf_w, door_top - FLOOR - 0.02)
    # -- The side and back walls: tall open windows below with walnut
    # mullions, dark glass above.
    wins = (15.4, 18.6, 21.8)
    win_hw, sill, head = 0.8, FLOOR + 0.7, FLOOR + 3.9
    for s in (-1, 1):
        x0, x1 = sorted((s * hw, s * (hw - t)))
        edges = [fy] + [v for y in wins for v in (y - win_hw, y + win_hw)] + [by]
        for ya, yb in zip(edges[0::2], edges[1::2]):
            box(b, (x0, ya, sill), (x1, yb, head), "lime", skip="-z +z", name="Wall")
        box(b, (x0, fy, FLOOR), (x1, by, sill), "lime", skip="-z -y +y", name="Wall")
        box(b, (x0, fy, head), (x1, by, WALL_TOP), "lime", skip="+z -y +y", name="Wall")
        outer = s * hw
        for y in wins:
            # The opening's walnut frame and mullions.
            if FAR:
                box(b, (min(outer, outer - s * 0.1), y - win_hw, sill),
                    (max(outer, outer - s * 0.1), y + win_hw, head), "glass", skip=only("+x" if s > 0 else "-x"),
                    name="Glass")
            else:
                fx0, fx1 = sorted((outer - s * 0.08, outer - s * 0.2))
                box(b, (fx0, y - 0.04, sill), (fx1, y + 0.04, head), "walnut", skip="-z +z", name="Mullion")
                for z in (sill + 1.1, sill + 2.2):
                    box(b, (fx0, y - win_hw, z - 0.04), (fx1, y + win_hw, z + 0.04), "walnut", skip="-y +y",
                        name="Mullion")
                sx0, sx1 = sorted((outer + s * 0.12, outer - s * 0.1))
                box(b, (sx0, y - win_hw - 0.12, sill - 0.1), (sx1, y + win_hw + 0.12, sill), "shade",
                    name="Sill")
            # Upper windows: dark glass with a bronze surround.
            gx0, gx1 = sorted((outer, outer + s * 0.04))
            box(b, (gx0, y - 0.65, CEILING + 0.9), (gx1, y + 0.65, WALL_TOP - 0.6), "glass", skip="-x" if s > 0
                else "+x", name="Glass")
        b.collide("wall", (x0 - 0.05, fy, FLOOR), (x1 + 0.05, by, WALL_TOP))
    box(b, (-hw + t, by - t, FLOOR), (hw - t, by, WALL_TOP), "lime", skip="-z +z -x +x", name="Wall")
    for x in (-6.0, -2.0, 2.0, 6.0):
        box(b, (x - 0.65, by, CEILING + 0.9), (x + 0.65, by + 0.04, WALL_TOP - 0.6), "glass", skip="-y",
            name="Glass")
    b.collide("facade", (-hw, fy, FLOOR), (-door_hw - 0.1, fy + t, WALL_TOP))
    b.collide("facade", (door_hw + 0.1, fy, FLOOR), (hw, fy + t, WALL_TOP))
    b.collide("back", (-hw, by - t, FLOOR), (hw, by, WALL_TOP))
    # -- The crown: the entablature round the whole plan, the attic, the
    # flat roof, and the chimneys.
    entablature(b, -hw, hw, COLUMN_Y - COLUMN_D / 2 - 0.05, by, WALL_TOP)
    if not FAR:
        # The dedication on the architrave, over the door.
        inscription(b, "HOUSE OF THE OWNER", COLUMN_Y - COLUMN_D / 2 - 0.05, WALL_TOP + 0.12)
    crown = WALL_TOP + ENTABLATURE
    box(b, (-hw + 0.5, COLUMN_Y + 0.2, crown), (hw - 0.5, by - 0.5, crown + ATTIC), "lime", skip="-z",
        name="Attic")
    box(b, (-hw + 0.42, COLUMN_Y + 0.12, crown + ATTIC - 0.16), (hw - 0.42, by - 0.42, crown + ATTIC), "shade",
        skip="-z", name="AtticCap")
    roof = crown + ATTIC
    chimney(b, -7.0, 13.4, roof, roof + 2.4)
    chimney(b, 7.0, 13.4, roof, roof + 2.4)
    chimney(b, -3.6, 22.8, roof, roof + 1.7, w=1.6, d=1.0)
    chimney(b, -1.8, 22.8, roof, roof + 1.2, w=1.2, d=0.9)
    b.roofs.append(((0.0, (COLUMN_Y + by) / 2), True, ((by - COLUMN_Y) / 2, hw - 0.5), roof, roof + 0.01))
    # The ceiling slab's underside, over the great room and under the
    # upper floor.
    slab(b, -hw + t, hw - t, fy + t, by - t, CEILING, "shade", down=True, name="Ceiling")
    # The forecourt's table and stools.
    if not FAR:
        bench_long(b, 0.0, 3.5, FORECOURT)
    for s in (-1, 1):
        planter(b, s * 3.6, y_portico + 0.9, FLOOR, size=0.9, ball=1.0)
    b.front = (0.0, -1.0)
    return y_portico


def great_room(b):
    """The enterable ground floor: coffers, pilasters, the dark wall with
    its engraved door and bookcases, a desk, a sofa, rugs, and a lamp."""
    hw, t = HALF_W, WALL
    fy, by = FRONT_WALL + t, BACK_WALL - t
    x0, x1 = -hw + t, hw - t
    if FAR:
        return
    coffers(b, x0, x1, fy, by, CEILING, xs=(-6.4, -3.2, 0.0, 3.2, 6.4), ys=(15.4, 18.6, 21.8))
    back = local((0, by, 0), 0.0)
    # The dark walnut wall, inscribed with faint circuit lines.
    box(b, (-6.0, -0.06, FLOOR), (6.0, 0.0, CEILING - 0.5), "walnut", skip="+y -z", xf=back, name="DarkWall")
    wall_face = local((0, by - 0.06, 0), 0.0)
    circuit_lines(b, wall_face, 12.0, FLOOR, CEILING - 0.5 - FLOOR)
    dentils(b, local((0, by, 0), 0.0), -9.4, 9.4, CEILING - 0.5)
    # The engraved door in a white marble surround with a cornice.
    dw, dh = 2.1, 3.5
    box(b, (-dw / 2 - 0.35, -0.12, FLOOR), (-dw / 2, 0.0, FLOOR + dh), "marble", skip="+y -z", xf=wall_face,
        name="Surround")
    box(b, (dw / 2, -0.12, FLOOR), (dw / 2 + 0.35, 0.0, FLOOR + dh), "marble", skip="+y -z", xf=wall_face,
        name="Surround")
    box(b, (-dw / 2 - 0.35, -0.12, FLOOR + dh), (dw / 2 + 0.35, 0.0, FLOOR + dh + 0.3), "marble", skip="+y",
        xf=wall_face, name="Surround")
    box(b, (-dw / 2 - 0.55, -0.26, FLOOR + dh + 0.3), (dw / 2 + 0.55, 0.0, FLOOR + dh + 0.5), "marble",
        skip="+y", xf=wall_face, name="Pediment")
    glyph(b, local((0, by - 0.12, FLOOR), 0.0), dw, dh)
    for u in (-3.7, 3.7):
        bookshelf(b, wall_face, u, FLOOR)
    # Marble pilasters: two on the dark wall's edges, and one on each pier
    # of the side walls between the windows.
    for u in (-6.45, 6.45):
        pilaster(b, back, u, FLOOR, CEILING - 0.5)
    for s in (-1, 1):
        side = local((s * (hw - t), 0, 0), -90 * s)
        for y in (13.6, 17.0, 20.2, 23.5):
            pilaster(b, side, -y * s, FLOOR, CEILING - 0.5)
    # The study: a rug, the desk facing the room, and a lamp.
    rug(b, -3.0, 3.0, 19.6, 23.8, FLOOR)
    desk(b, 0.0, 21.6, FLOOR + 0.02)
    lamp(b, 4.6, 23.6, FLOOR)
    # The sitting room: a long sofa facing the dark wall across a low
    # table, on a second rug.
    rug(b, -3.6, 3.6, 13.4, 18.2, FLOOR)
    sofa(b, 0.0, 14.4, FLOOR + 0.02, rot=180.0)
    low_table(b, 0.0, 16.3, FLOOR + 0.02)
    planter(b, -8.6, 24.2, FLOOR, size=0.8, ball=1.2)
    planter(b, 8.6, 13.2, FLOOR, size=0.8, ball=1.2)
    b.inside = (0.0, 12.9)


def new(name):
    b = bl.Building(name, {"plaster": "cream", "roof": "red", "timber": "dark"})
    b.chimneys = []
    materials(b)
    return b


MODELS = {}


def model(fn):
    MODELS[fn.__name__] = fn
    return fn


@model
def greco_house():
    b = new("greco_house")
    house_body(b)
    great_room(b)
    return b


# -- Kit pieces, each alone, for review and later buildings.

KIT = {}


def piece(fn):
    KIT[fn.__name__] = fn
    return fn


@piece
def column_piece():
    b = new("column")
    column(b, 0, 0, 0, 8.2)
    return b


@piece
def pier_piece():
    b = new("pier")
    pier(b, 0, 0, 0, 8.2)
    return b


@piece
def entablature_bay():
    b = new("entablature_bay")
    entablature(b, -2.0, 2.0, 0.0, 1.2, 0.0, panels=("-y",), spacing=1.0)
    return b


@piece
def stair_flight():
    b = new("stair_flight")
    stair(b, -2.0, 2.0, 0.0, 0.0, 6, 1.0 / 6, 0.38)
    return b


@piece
def planter_wall_piece():
    b = new("planter_wall")
    planter_wall(b, -1.5, 1.5, 0.0, 1.2, 0.0, 0.6, hedge=0.8)
    return b


@piece
def circuit_door():
    b = new("circuit_door")
    box(b, (-1.35, 0.0, 0.0), (1.35, 0.3, 4.3), "bronze", skip="-z -y", name="Frame")
    glyph(b, local((0, 0.0, 0.0), 0.0), 2.4, 3.9)
    glyph(b, local((0, -0.02, 3.9), 0.0), 2.4, 0.4, tall=False, leaf=False)
    return b


@piece
def lattice_screen():
    b = new("lattice_screen")
    lattice(b, -1.7, 1.7, 0.0, 4.0, 0.0)
    return b


@piece
def coffer_bay():
    b = new("coffer_bay")
    coffers(b, -3.2, 3.2, 0.0, 6.4, 4.0, xs=(0.0,), ys=(3.2,))
    slab(b, -3.2, 3.2, 0.0, 6.4, 4.0, "lime", down=True, name="Ceiling")
    return b


@piece
def pilaster_piece():
    b = new("pilaster")
    pilaster(b, local((0, 0, 0)), 0.0, 0.0, 4.3)
    return b


@piece
def chimney_piece():
    b = new("chimney")
    chimney(b, 0, 0, 0, 2.4)
    return b


@piece
def circuit_panel():
    b = new("circuit_panel")
    box(b, (-3.0, -0.06, 0.0), (3.0, 0.0, 4.0), "walnut", skip="+y -z", name="DarkWall")
    circuit_lines(b, local((0, -0.06, 0)), 6.0, 0.0, 4.0)
    return b


@piece
def bench_piece():
    b = new("bench_long")
    bench_long(b, 0, 0, 0)
    return b


@piece
def planter_piece():
    b = new("planter")
    planter(b, 0, 0, 0)
    return b


@piece
def lamp_piece():
    b = new("lamp")
    lamp(b, 0, 0, 0)
    return b


@piece
def rug_piece():
    b = new("rug")
    rug(b, -2.0, 2.0, -1.5, 1.5, 0.0)
    return b


@piece
def desk_piece():
    b = new("desk")
    desk(b, 0, 0, 0)
    return b


@piece
def sofa_piece():
    b = new("sofa")
    sofa(b, 0, 0, 0)
    low_table(b, 0, -1.6, 0)
    return b


@piece
def bookshelf_piece():
    b = new("bookshelf")
    bookshelf(b, local((0, 0, 0)), 0.0, 0.0)
    return b


# --------------------------------------------------------------------------
# Saving


def join(b):
    objs = [o for o in b.col.objects if o.type == "MESH"]
    if os.environ.get("GRECO_TALLY"):
        tally = {}
        for o in objs:
            o.data.calc_loop_triangles()
            key = o.name.rsplit(".", 1)[0]
            tally[key] = tally.get(key, 0) + len(o.data.loop_triangles)
        print("TALLY", b.name, sorted(tally.items(), key=lambda kv: -kv[1])[:24])
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    obj = bpy.context.object
    obj.name = b.name
    used = {s.material.name for s in obj.material_slots if s.material}
    assert len(used) <= 16, sorted(used)
    return obj


def save(b, out):
    """One object, so the glTF has one node: the pack bounds a model's nodes."""
    join(b)
    b.save(out)
    for top in b.chimneys:
        print(f"CHIMNEY {b.name} {top[0]} {top[1]} {top[2]}")
    print(f"MODEL {b.name} {b.triangles()}")


def save_far(b, out):
    join(b)
    os.makedirs(out, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=os.path.join(out, b.name + ".glb"), export_format="GLB",
                              export_yup=True, export_apply=True, export_image_format="JPEG",
                              export_jpeg_quality=88, export_tangents=False, export_cameras=False,
                              export_lights=False, export_extras=False)
    print(f"FAR {b.name} triangles={b.triangles()}")


def main():
    global FAR
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if "--kit" in args:
        i = args.index("--kit")
        bl.KIT = os.path.expanduser(args[i + 1])
        del args[i:i + 2]
    out = args[0] if args else os.path.join(os.path.dirname(__file__), "..", "..", "assets", "verse",
                                            "generated", "greco")
    names = args[1:] or (list(MODELS) + list(KIT))
    for name in names:
        if name in MODELS:
            FAR = False
            save(MODELS[name](), out)
            FAR = True
            save_far(MODELS[name](), os.path.join(out, "far"))
            FAR = False
        else:
            b = KIT[name]()
            save(b, os.path.join(out, "kit"))
            # The footprint of a kit piece isn't used; keep the glb alone.
            os.remove(os.path.join(out, "kit", b.name + ".footprint.json"))


main()
