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
  bookshelves, a desk, a reception desk and chair by the door, a long
  sofa, rugs, a lamp, and a planter, lit by
  candles, sconces, a brazier, lamps, and lanterns whose lights and flames
  its footprint records. It also writes `far/greco_house.glb`, the far
  level of detail.
- `civic_hall`: the Civic Hall, from a fifth reference image: a broad
  two-storey civic building on a podium, up twelve shallow steps between
  walnut planter walls with bronze bowls, with four bronze-banded columns
  on high plinths before a projecting pavilion, a deep dentil cornice
  under a stepped attic, paired tall windows on its wings, and a very tall
  copper portal carrying the seal, whose inset door stands open. Inside,
  the council chamber: a ring of tiered benches round a well with the seal
  inlaid in its floor, the speaker's dais, and candlelight. It also writes
  `far/civic_hall.glb`.
- `belvedere`: from a sixth and a seventh reference image: a loggia on a
  terrace up twenty steps, opening through marble piers inlaid with
  bronze lines under copper corbels, a red-brown lintel band inlaid in
  copper, a coffered marble ceiling with copper lines, walnut louvers, a
  copper relief of abstract figures over a cushioned bench, urn trees, and
  a walnut side door; and behind it an entry court whose colonnade frames
  a mahogany double door in a stepped bronze surround over a meander
  floor, with terracotta pots. It also writes `far/belvedere.glb`.
- Kit pieces under `kit/`, for review and later buildings: `column`,
  `pier`, `entablature_bay`, `stair_flight`, `planter_wall`,
  `circuit_door`, `lattice_screen`, `coffer_bay`, `pilaster`, `chimney`,
  `circuit_panel`, `bench_long`, `planter`, `lamp`, `rug`, `desk`, `sofa`,
  `bookshelf`, `reception_desk`, `reception_chair`, and the Civic Hall's `bronze_column`, `dentil_cornice_bay`,
  `attic`, `circuit_portal`, `paired_window`, `bowl_planter`, and
  `council_ring`; and the belvedere's `inlaid_pier`, `lintel_band`,
  `louver`, `relief_panel`, `cushioned_bench`, `urn_tree`, `threshold`,
  `mahogany_door`, `stepped_surround`, `meander_floor`, and
  `terracotta_pot`.
"""

import json
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
# Emitted colors: Everglade draws an `Emit...` material at 6,000 cd/m^2
# times its linear color, so these set how brightly each glows: flames
# past white, the cove's line soft, the inlay a faint ember.
FLAME = (1.00, 0.82, 0.50)
COVE = (0.30, 0.22, 0.12)
INLAY = (0.20, 0.08, 0.03)

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
    # Light the house gives (`scene::emits` draws `Emit...` glowing): candle
    # and brazier flames, the dark wall's circuit inlay, low, and the warm
    # line of light in the coffers' cove.
    m["flame"] = bl._material("EmitFlame", color=FLAME, rough=0.5, emit=FLAME)
    m["inlay"] = bl._material("EmitInlay", color=INLAY, rough=0.4, emit=INLAY)
    m["cove"] = bl._material("EmitCove", color=COVE, rough=0.5, emit=COVE)


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


def dentils(b, xf, u0, u1, z, tooth=0.12, gap=0.14, out=0.12, tall=0.16, mat="marble"):
    """A row of dentils under a cornice on a wall's face."""
    u = u0
    while u + tooth <= u1:
        box(b, (u, -out, z - tall), (u + tooth, 0.0, z), mat, skip="+y +z -x +x", xf=xf, name="Dentil")
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
                box(b, (ua - 0.015, -0.012, z_0 - 0.015), (ub + 0.015, 0.0, z_1 + 0.015), "inlay",
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


def workstation(b, x, y, z0, rot=0.0):
    """The workshop agent's workstation: a long walnut desk on two
    pedestals with a copper edge, a front panel inscribed with circuit
    lines toward the room, three slim bronze-framed screens glowing amber
    toward her chair, a keyboard, and her chair. The screens stay low, so
    the room sees her over them."""
    xf = local((x, y, z0), rot)
    w, d, h = 2.96, 1.0, 0.76
    box(b, (-w / 2, -d / 2, h - 0.06), (w / 2, d / 2, h), "walnut", xf=xf, name="DeskTop")
    box(b, (-w / 2 - 0.02, -d / 2 - 0.02, h - 0.09), (w / 2 + 0.02, d / 2 + 0.02, h - 0.06), "copper", skip="+z",
        xf=xf, name="DeskEdge")
    for s in (-1, 1):
        box(b, (s * 1.08 - 0.3, -d / 2 + 0.08, 0.0), (s * 1.08 + 0.3, d / 2 - 0.08, h - 0.09), "walnut",
            skip="-z +z", xf=xf, name="Pedestal")
    # The front panel, between the pedestals, toward the room.
    box(b, (-0.78, -d / 2 + 0.04, 0.16), (0.78, -d / 2 + 0.08, h - 0.09), "walnut", skip="+y -z +z", xf=xf,
        name="DeskPanel")
    b.collide("workstation", Vector(xf @ Vector((-w / 2, -d / 2, 0))), Vector(xf @ Vector((w / 2, d / 2, h))))
    if FAR:
        return
    circuit_lines(b, xf @ Matrix.Translation(Vector((0.0, -d / 2 + 0.04, 0.0))), 1.5, 0.2, h - 0.33)
    # Three slim screens toward her chair, the side ones turned in.
    for u, turn in ((-0.94, 18.0), (0.0, 0.0), (0.94, -18.0)):
        sx = xf @ Matrix.Translation(Vector((u, -0.22, h))) @ Matrix.Rotation(math.radians(turn), 4, "Z")
        box(b, (-0.46, -0.03, 0.07), (0.46, 0.0, 0.38), "bronze", skip="-z", xf=sx, name="ScreenFrame")
        box(b, (-0.43, 0.0, 0.1), (0.43, 0.004, 0.35), "amber", skip=only("+y"), xf=sx, name="EmitScreen")
        box(b, (-0.025, -0.06, 0.0), (0.025, -0.03, 0.07), "copper", skip="-z +z", xf=sx, name="ScreenStand")
    # The keyboard, with an amber line of keys.
    box(b, (-0.36, 0.12, h), (0.36, 0.32, h + 0.02), "bronze", skip="-z", xf=xf, name="Keyboard")
    box(b, (-0.3, 0.17, h + 0.02), (0.3, 0.27, h + 0.021), "amber", skip=only("+z"), xf=xf, name="EmitKeys")
    # Her chair, behind the desk, facing the room.
    box(b, (-0.28, 0.9, 0.42), (0.28, 1.4, 0.5), "redbrown", xf=xf, name="Chair")
    box(b, (-0.28, 1.35, 0.5), (0.28, 1.43, 1.0), "redbrown", xf=xf, name="Chair")
    box(b, (-0.03, 1.12, 0.0), (0.03, 1.18, 0.42), "copper", skip="-z +z", xf=xf, name="ChairPost")
    box(b, (-0.3, 0.95, 0.0), (0.3, 1.35, 0.03), "copper", skip="-z", xf=xf, name="ChairFoot")


def console(b, xf, h=1.0, w=1.4, d=0.75):
    """A standing walnut console against a wall on a limestone plinth,
    with an inclined amber screen: where the workshop agent works while a
    command runs."""
    box(b, (-w / 2, -d, 0.0), (w / 2, 0.0, 0.12), "lime", skip="+y -z", xf=xf, name="ConsolePlinth")
    box(b, (-w / 2 + 0.05, -d + 0.05, 0.12), (w / 2 - 0.05, 0.0, h - 0.05), "walnut", skip="+y -z", xf=xf,
        name="Console")
    box(b, (-w / 2, -d, h - 0.05), (w / 2, 0.0, h), "bronze", skip="+y", xf=xf, name="ConsoleTop")
    if FAR:
        return
    circuit_lines(b, xf @ Matrix.Translation(Vector((0.0, -d + 0.05, 0.0))), w - 0.2, 0.2, h - 0.35)
    tilt = xf @ Matrix.Translation(Vector((0.0, -0.25, h))) @ Matrix.Rotation(math.radians(-25.0), 4, "X")
    box(b, (-0.5, -0.03, 0.0), (0.5, 0.0, 0.5), "bronze", skip="-z", xf=tilt, name="ScreenFrame")
    box(b, (-0.46, -0.034, 0.04), (0.46, -0.03, 0.46), "amber", skip=only("-y"), xf=tilt, name="EmitScreen")


def lectern(b, x, y, z0, rot=0.0):
    """A limestone lectern with a bronze top and an amber slit toward the
    room: where the workshop agent waits for an approval."""
    xf = local((x, y, z0), rot)
    box(b, (-0.22, -0.22, 0.0), (0.22, 0.22, 1.0), "lime", skip="-z", xf=xf, name="Lectern")
    box(b, (-0.28, -0.28, 1.0), (0.28, 0.28, 1.06), "bronze", xf=xf, name="LecternTop")
    b.collide("lectern", Vector(xf @ Vector((-0.25, -0.25, 0))), Vector(xf @ Vector((0.25, 0.25, 1.06))))
    if FAR:
        return
    box(b, (-0.14, -0.222, 0.86), (0.14, -0.22, 0.9), "amber", skip=only("-y"), xf=xf, name="EmitSlit")


def collide_turned(b, name, xf, lo, hi):
    """Records the axis-aligned box round a box from `lo` to `hi` in the
    frame `xf`, which may be turned, rounded out to the centimeter."""
    corners = [xf @ Vector((x, y, z)) for x in (lo[0], hi[0]) for y in (lo[1], hi[1]) for z in (lo[2], hi[2])]
    low = Vector([math.floor(min(c[i] for c in corners) * 100) / 100 for i in range(3)])
    high = Vector([math.ceil(max(c[i] for c in corners) * 100) / 100 for i in range(3)])
    b.collide(name, low, high)


# The reception chair's seat height, m: the seated private character's
# `seat_m` (`private_character.py --pose seated`) for a 1.62 m body, 0.395 m.
RECEPTION_SEAT = 0.40
# The reception chair in the great room, x and y, and its turn, degrees,
# toward the door's middle at (0, 12.4) (`layout::estate::RECEPTION`).
RECEPTION = (-3.2, 16.2)
RECEPTION_TURN = 40.0


def reception_desk(b, x, y, z0, rot=0.0):
    """The reception desk: a walnut writing top at desk height over walnut
    ends, and toward the visitor a tall walnut front inlaid with circuit
    lines over a copper foot band, under a bronze ledge. An amber slate
    glows on the top. The receptionist sits behind it, at +y, facing -y."""
    xf = local((x, y, z0), rot)
    w, d, h, front = 1.8, 0.75, 0.76, 0.88
    box(b, (-w / 2 + 0.06, -d / 2 + 0.1, h - 0.05), (w / 2 - 0.06, d / 2, h), "walnut", xf=xf, name="DeskTop")
    for s in (-1, 1):
        u0, u1 = sorted((s * w / 2, s * (w / 2 - 0.06)))
        box(b, (u0, -d / 2, 0.0), (u1, d / 2, h), "walnut", skip="-z", xf=xf, name="DeskEnd")
    box(b, (-w / 2, -d / 2, 0.08), (w / 2, -d / 2 + 0.1, front), "walnut", skip="-z", xf=xf, name="DeskFront")
    box(b, (-w / 2 - 0.01, -d / 2 - 0.01, 0.0), (w / 2 + 0.01, -d / 2 + 0.1, 0.08), "copper", skip="-z", xf=xf,
        name="DeskFoot")
    box(b, (-w / 2 - 0.03, -d / 2 - 0.07, front), (w / 2 + 0.03, -d / 2 + 0.22, front + 0.04), "bronze", xf=xf,
        name="Ledge")
    collide_turned(b, "reception desk", xf, (-w / 2, -d / 2 - 0.07, 0.0), (w / 2, d / 2, front + 0.04))
    if FAR:
        return
    circuit_lines(b, xf @ Matrix.Translation(Vector((0.0, -d / 2, 0.0))), w - 0.3, 0.14, front - 0.32)
    box(b, (-0.24, 0.02, h), (0.24, 0.3, h + 0.016), "bronze", skip="-z", xf=xf, name="Slate")
    box(b, (-0.21, 0.05, h + 0.016), (0.21, 0.27, h + 0.017), "amber", skip=only("+z"), xf=xf, name="EmitSlate")


def reception_chair(b, x, y, z0, rot=0.0, seat=RECEPTION_SEAT):
    """The receptionist's chair: a walnut seat and back with red-brown
    cushions on four bronze legs, a copper circuit trace on the back's
    outer face. She sits on it facing -y."""
    xf = local((x, y, z0), rot)
    w, d, back = 0.52, 0.5, 0.52
    box(b, (-w / 2, -d / 2, seat - 0.08), (w / 2, d / 2, seat - 0.03), "walnut", xf=xf, name="ChairSeat")
    box(b, (-w / 2 + 0.03, -d / 2 + 0.03, seat - 0.03), (w / 2 - 0.03, d / 2 - 0.08, seat), "redbrown",
        skip="-z", xf=xf, name="Cushion")
    box(b, (-w / 2, d / 2 - 0.05, seat - 0.03), (w / 2, d / 2, seat + back), "walnut", xf=xf, name="ChairBack")
    box(b, (-w / 2 + 0.05, d / 2 - 0.08, seat + 0.08), (w / 2 - 0.05, d / 2 - 0.05, seat + back - 0.06),
        "redbrown", skip="+y", xf=xf, name="Cushion")
    for sx in (-1, 1):
        for sy in (-1, 1):
            cx, cy = sx * (w / 2 - 0.05), sy * (d / 2 - 0.05)
            box(b, (cx - 0.02, cy - 0.02, 0.0), (cx + 0.02, cy + 0.02, seat - 0.08), "bronze", skip="-z +z",
                xf=xf, name="ChairLeg")
    collide_turned(b, "reception chair", xf, (-w / 2, -d / 2, 0.0), (w / 2, d / 2, seat + back))
    if FAR:
        return
    rear = xf @ Matrix.Translation(Vector((0.0, d / 2, 0.0))) @ Matrix.Rotation(math.pi, 4, "Z")
    circuit_lines(b, rear, w - 0.12, seat + 0.04, back - 0.1)


def reception(b, x, y, z0, rot=0.0):
    """The reception: the chair at (x, y), facing along `rot` as a kit
    piece faces, and the desk before it, far enough ahead that her knees
    fit under its top."""
    reception_chair(b, x, y, z0, rot)
    a = math.radians(rot)
    ahead = 0.62
    reception_desk(b, x + ahead * math.sin(a), y - ahead * math.cos(a), z0, rot)


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
    if hasattr(b, "lights"):
        light(b, "lamp", (x, y, z0 + height - 0.15))


def light(b, kind, at):
    """Records a light the zone gives the house (`layout::estate`): its
    kind and where, in Blender coordinates."""
    b.lights.append((kind, tuple(round(v, 3) for v in at)))


def flame(b, x, y, z, h=0.08, r=0.024):
    """A candle flame: a small glowing cone, and a halo the zone draws."""
    if FAR:
        return
    b.solid("Flame", bl.frustum_mesh("Flame", (x, y), r, r * 0.15, z, z + h, 5, b.mats["flame"]))
    b.flames.append((round(x, 3), round(y, 3), round(z + h * 0.45, 3)))


def candle(b, x, y, z, h=0.22, r=0.026):
    """A linen-white candle with its flame; returns the flame's height."""
    prism(b, (x, y), r, z, z + h, 6, "linen", name="Candle", cap=True)
    flame(b, x, y, z + h)
    return z + h + 0.05


def candelabra(b, x, y, z, arms=0.17, stem=0.32):
    """A bronze candelabrum of three candles on a desk or a table."""
    prism(b, (x, y), 0.09, z, z + 0.03, 8, "bronze", name="Candelabra", cap=True)
    box(b, (x - 0.015, y - 0.015, z + 0.03), (x + 0.015, y + 0.015, z + stem), "bronze", skip="-z",
        name="Candelabra")
    box(b, (x - arms, y - 0.015, z + stem - 0.02), (x + arms, y + 0.015, z + stem), "bronze",
        name="Candelabra")
    top = z + stem
    for dx, h in ((-arms, 0.18), (0.0, 0.24), (arms, 0.18)):
        prism(b, (x + dx, y), 0.035, top, top + 0.025, 6, "bronze", name="Cup", cap=True)
        flame_z = candle(b, x + dx, y, top + 0.025, h=h)
    light(b, "candles", (x, y, flame_z))


def floor_candelabrum(b, x, y, z0, height=1.5):
    """A tall bronze candle stand on the floor, three candles high."""
    box(b, (x - 0.2, y - 0.2, z0), (x + 0.2, y + 0.2, z0 + 0.05), "bronze", skip="-z", name="Stand")
    box(b, (x - 0.025, y - 0.025, z0 + 0.05), (x + 0.025, y + 0.025, z0 + height - 0.3), "bronze",
        skip="-z +z", name="Stand")
    candelabra(b, x, y, z0 + height - 0.3, arms=0.2, stem=0.05)


def sconce(b, xf, u, z):
    """A bronze wall sconce on a wall's face: a backplate, an arm, a cup,
    and a candle."""
    box(b, (u - 0.08, -0.04, z - 0.24), (u + 0.08, 0.0, z + 0.2), "bronze", skip="+y", xf=xf, name="Sconce")
    box(b, (u - 0.02, -0.26, z - 0.02), (u + 0.02, -0.04, z + 0.02), "bronze", xf=xf, name="Sconce")
    box(b, (u - 0.07, -0.33, z - 0.05), (u + 0.07, -0.19, z + 0.02), "bronze", xf=xf, name="Sconce")
    if FAR:
        return
    c = xf @ Vector((u, -0.26, z + 0.02))
    top = candle(b, c.x, c.y, c.z, h=0.2)
    light(b, "sconce", (c.x, c.y, top))


def lantern(b, x, y, z, w=0.24, h=0.34):
    """A square bronze lantern with amber glass and a stepped cap; its
    light the zone gives. The far level keeps only the lit glass."""
    if FAR:
        box(b, (x - w / 2, y - w / 2, z + 0.05), (x + w / 2, y + w / 2, z + 0.05 + h), "amber", skip="-z",
            name="LanternGlass")
        light(b, "lantern", (x, y, z + 0.05 + h / 2))
        return
    box(b, (x - w / 2 - 0.03, y - w / 2 - 0.03, z), (x + w / 2 + 0.03, y + w / 2 + 0.03, z + 0.05), "bronze",
        name="Lantern")
    box(b, (x - w / 2, y - w / 2, z + 0.05), (x + w / 2, y + w / 2, z + 0.05 + h), "amber", skip="-z +z",
        name="LanternGlass")
    box(b, (x - w / 2 - 0.04, y - w / 2 - 0.04, z + 0.05 + h), (x + w / 2 + 0.04, y + w / 2 + 0.04,
                                                                  z + 0.1 + h), "bronze", name="Lantern")
    box(b, (x - 0.06, y - 0.06, z + 0.1 + h), (x + 0.06, y + 0.06, z + 0.18 + h), "bronze", skip="-z",
        name="Lantern")
    light(b, "lantern", (x, y, z + 0.05 + h / 2))


def lantern_post(b, x, y, z0, height=1.8):
    """A bronze post on a square foot with a lantern on top."""
    if FAR:
        box(b, (x - 0.045, y - 0.045, z0), (x + 0.045, y + 0.045, z0 + height), "bronze", skip="-z +z",
            name="Post")
        lantern(b, x, y, z0 + height)
        return
    box(b, (x - 0.12, y - 0.12, z0), (x + 0.12, y + 0.12, z0 + 0.12), "bronze", skip="-z", name="Post")
    box(b, (x - 0.045, y - 0.045, z0 + 0.12), (x + 0.045, y + 0.045, z0 + height), "bronze", skip="-z +z",
        name="Post")
    lantern(b, x, y, z0 + height)
    b.collide("lantern post", (x - 0.12, y - 0.12, z0), (x + 0.12, y + 0.12, z0 + height + 0.5))


def wall_lantern(b, x, y_face, z, out=0.38):
    """A lantern hung on a bracket from a pier's or a wall's front face
    at y_face (facing -y)."""
    if FAR:
        lantern(b, x, y_face - out, z)
        return
    box(b, (x - 0.06, y_face - 0.04, z + 0.25), (x + 0.06, y_face, z + 0.6), "bronze", skip="+y",
        name="Bracket")
    box(b, (x - 0.02, y_face - out, z + 0.54), (x + 0.02, y_face - 0.04, z + 0.58), "bronze", name="Bracket")
    lantern(b, x, y_face - out, z)


def uplight(b, x, y, z0, toward=(0.0, 1.0)):
    """A low bronze uplight on the floor, its warm face up, washing the
    stone beside it; the zone's light stands a little off its face."""
    if FAR:
        light(b, "uplight", (x + toward[0] * 0.3, y + toward[1] * 0.3, z0 + 0.9))
        return
    box(b, (x - 0.16, y - 0.12, z0), (x + 0.16, y + 0.12, z0 + 0.12), "bronze", skip="-z", name="Uplight")
    if not FAR:
        box(b, (x - 0.12, y - 0.08, z0 + 0.12), (x + 0.12, y + 0.08, z0 + 0.125), "cove", skip="-z -x +x -y +y",
            name="Uplight")
    light(b, "uplight", (x + toward[0] * 0.3, y + toward[1] * 0.3, z0 + 0.9))


def brazier(b, x, y, z0):
    """A bronze tripod brazier with a fire burning in its bowl."""
    for k in range(3):
        a = 2 * math.pi * k / 3
        lx, ly = x + 0.3 * math.cos(a), y + 0.3 * math.sin(a)
        box(b, (lx - 0.03, ly - 0.03, z0), (lx + 0.03, ly + 0.03, z0 + 0.78), "bronze", skip="-z",
            name="BrazierLeg")
    b.solid("Bowl", bl.frustum_mesh("Bowl", (x, y), 0.2, 0.44, z0 + 0.72, z0 + 0.95, 8, b.mats["bronze"],
                                    cap=True))
    if not FAR:
        for dx, dy, h, r in ((0.0, 0.0, 0.42, 0.13), (0.14, 0.06, 0.28, 0.09), (-0.12, 0.08, 0.3, 0.09),
                             (0.02, -0.14, 0.26, 0.08)):
            b.solid("Fire", bl.frustum_mesh("Fire", (x + dx, y + dy), r, r * 0.1, z0 + 0.93, z0 + 0.93 + h, 5,
                                            b.mats["flame"]))
        b.flames.append((round(x, 3), round(y, 3), round(z0 + 1.08, 3)))
    light(b, "brazier", (x, y, z0 + 1.35))
    b.collide("brazier", (x - 0.45, y - 0.45, z0), (x + 0.45, y + 0.45, z0 + 1.0))


def side_table(b, x, y, z0):
    """A round walnut side table on a bronze pedestal, with two candles."""
    prism(b, (x, y), 0.3, z0 + 0.52, z0 + 0.57, 8, "walnut", name="SideTable", cap=True)
    box(b, (x - 0.03, y - 0.03, z0), (x + 0.03, y + 0.03, z0 + 0.52), "bronze", skip="-z +z", name="SideTable")
    prism(b, (x, y), 0.18, z0, z0 + 0.03, 8, "bronze", name="SideTable", cap=True)
    top = z0 + 0.57
    candle(b, x - 0.08, y, top, h=0.24)
    candle(b, x + 0.09, y + 0.05, top, h=0.17)
    light(b, "candles", (x, y, top + 0.3))


def cove_light(b, x0, x1, y0, y1, z, inset=0.45, width=0.1):
    """A warm line of light under the cove's second step, round the room."""
    if FAR:
        return
    for lo, hi in (((x0 + inset, y0 + inset, z), (x1 - inset, y0 + inset + width, z + 0.01)),
                   ((x0 + inset, y1 - inset - width, z), (x1 - inset, y1 - inset, z + 0.01)),
                   ((x0 + inset, y0 + inset + width, z), (x0 + inset + width, y1 - inset - width, z + 0.01)),
                   ((x1 - inset - width, y0 + inset + width, z), (x1 - inset, y1 - inset - width, z + 0.01))):
        box(b, lo, hi, "cove", skip="+z -x +x -y +y", name="CoveLight")


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
    # The threshold: the podium's top through the doorway, between the
    # portico and the great room's floor. Everglade's collision also fills
    # it (`layout::generated::PATCHES`).
    slab(b, -door_hw, door_hw, fy, fy + t, FLOOR, "marble", name="Threshold")
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
    # Light: lanterns on the inner piers' faces, uplights washing the door
    # and its screens, and lanterns on posts beside both flights.
    for s in (-1, 1):
        wall_lantern(b, s * 6.6, COLUMN_Y - COLUMN_D / 2, FLOOR + 3.1)
        uplight(b, s * 1.9, fy - 0.35, FLOOR, toward=(0.0, -1.0))
        lantern_post(b, s * 5.0, -0.6, 0.0)
        lantern_post(b, s * 4.85, upper_y - 0.4, FORECOURT, height=1.6)
    # The forecourt's table and stools, to one side of the walk up the stairs.
    if not FAR:
        bench_long(b, -6.6, 3.6, FORECOURT)
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
    # The workshop agent's workstation, at the spot kept for it west of the
    # desk, facing the room, with a console by the east wall where she
    # works while a command runs and a lectern where she waits for an
    # approval, all off the walk from the door.
    workstation(b, -4.6, 22.6, FLOOR)
    east = local((hw - t, 21.6, FLOOR), -90.0)
    console(b, east)
    b.collide("console", Vector((hw - t - 0.75, 20.9, FLOOR)), Vector((hw - t, 22.3, FLOOR + 1.0)))
    lectern(b, 5.0, 15.0, FLOOR)
    # The reception, west of the entry walk a few strides in from the
    # door: a chair turned toward the door, so whoever sits there greets
    # arrivals, with its desk before it, both clear of the walk.
    reception(b, *RECEPTION, FLOOR, RECEPTION_TURN)
    # Candlelight: a candelabrum on the desk, sconces flanking the engraved
    # door, a tall candle stand lighting the workstation spot west of the
    # desk (kept clear for a seat facing into the room), a brazier across
    # from the sofa, and the cove's warm line round the coffers.
    candelabra(b, 0.55, 21.85, FLOOR + 0.02 + 0.76)
    for u in (-1.85, 1.85):
        sconce(b, wall_face, u, FLOOR + 2.3)
    floor_candelabrum(b, -6.3, 24.3, FLOOR)
    brazier(b, 7.4, 18.6, FLOOR)
    # The door end: sconces flanking the door on the facade's inner face,
    # and candle stands in the front corners, so the room reads candlelit
    # from the entrance too.
    door_face = local((0, fy, 0), 180.0)
    for u in (-1.95, 1.95):
        sconce(b, door_face, u, FLOOR + 2.3)
    floor_candelabrum(b, -8.4, 12.9, FLOOR)
    floor_candelabrum(b, 7.5, 12.9, FLOOR)
    cove_light(b, x0, x1, fy, by, CEILING - 0.285)
    # The sitting room, off the entry: a long sofa with its back to the
    # west wall, facing across the room past a low table, on a second rug.
    # The walk from the door to the desk stays clear, at least 2.5 m wide.
    rug(b, -9.4, -5.6, 15.8, 21.4, FLOOR)
    sofa(b, -8.75, 18.6, FLOOR + 0.02, rot=90.0)
    low_table(b, -7.0, 18.6, FLOOR + 0.02, rot=90.0)
    side_table(b, -8.7, 15.7, FLOOR)
    lamp(b, -8.8, 21.7, FLOOR)
    planter(b, -8.6, 24.2, FLOOR, size=0.8, ball=1.2)
    planter(b, 8.6, 13.2, FLOOR, size=0.8, ball=1.2)
    b.inside = (0.0, 18.0)


# --------------------------------------------------------------------------
# The Civic Hall's kit, from the fifth reference image: a broad civic
# building with a projecting pavilion, columns on high plinths with bronze
# bands, a deep dentil cornice under a stepped attic, paired tall windows,
# and a very tall circuit-relief portal with a small door at its foot.


def trace(b, xf, p, q, width=0.05, proud=0.025, mat="bronze", name="Trace"):
    """A straight inlay from `p` to `q`, each (u, z) on a face in the local
    plane y = 0 facing -y, at any angle: one face, 2 triangles."""
    (u0, z0), (u1, z1) = p, q
    length = math.hypot(u1 - u0, z1 - z0)
    if length < 1e-6:
        return
    a = math.atan2(z1 - z0, u1 - u0)
    m = (xf @ Matrix.Translation(Vector(((u0 + u1) / 2, 0.0, (z0 + z1) / 2)))
         @ Matrix.Rotation(-a, 4, "Y"))
    box(b, (-length / 2 - width / 2, -proud, -width / 2), (length / 2 + width / 2, 0.0, width / 2), mat,
        skip=only("-y"), xf=m, name=name)


def polyline(b, xf, pts, mirror=True, **kw):
    """Inlays along `pts`, and their mirror across u = 0."""
    for m in ((1, -1) if mirror else (1,)):
        q = [(u * m, z) for u, z in pts]
        for p0, p1 in zip(q, q[1:]):
            trace(b, xf, p0, p1, **kw)


def arc(b, xf, c, r, a0, a1, n, **kw):
    """An arc of inlay round `c` from angle `a0` to `a1` (radians, from +u
    toward +z), in `n` straight pieces."""
    pts = [(c[0] + r * math.cos(a0 + (a1 - a0) * i / n), c[1] + r * math.sin(a0 + (a1 - a0) * i / n))
           for i in range(n + 1)]
    for p0, p1 in zip(pts, pts[1:]):
        trace(b, xf, p0, p1, **kw)


# The floor's frame for `trace`: the local plane y = 0 laid flat, facing
# up, with u along x and z along y.
FLAT = Matrix.Rotation(math.radians(-90.0), 4, "X")


def seal(b, xf, w, h, inset=(0.0, 0.0), mat="bronze"):
    """The Civic Hall's relief on a panel `w` wide and `h` tall, in metres
    (the local plane y = 0, facing -y, u from -w/2, z from 0): a border, a
    stepped head between two bands, the circle-and-cross glyph with stepped
    arches in its lower half, three amber lights under it, stepped
    shoulders falling to the edges, and bands round the inset door
    (`inset`: its width and height, or none)."""
    if FAR:
        return
    kw = {"mat": mat}
    e, top = w / 2 - 0.16, h - 0.14
    polyline(b, xf, [(e, 0.12), (e, top), (0.0, top)], **kw)
    # The head: a band, and steps rising to the spine.
    zb = h * 0.84
    za, zc = zb + 0.32 * (top - zb), zb + 0.66 * (top - zb)
    polyline(b, xf, [(e, zb), (0.0, zb)], **kw)
    polyline(b, xf, [(e, za), (w * 0.3, za), (w * 0.3, zc), (w * 0.14, zc), (w * 0.14, top)], **kw)
    # The circle and its cross: a spine from under the circle up to the
    # head's band, a bar whose ends turn down, and two half rings stepping
    # down inside the circle's lower half.
    r, cz = w * 0.23, h * 0.64
    arc(b, xf, (0.0, cz), r, 0.0, 2 * math.pi, 28, **kw)
    for k in (0.74, 0.5):
        arc(b, xf, (0.0, cz), r * k, math.pi, 2 * math.pi, 14, **kw)
    spine = cz - r * 1.18
    trace(b, xf, (0.0, spine), (0.0, zb), **kw)
    polyline(b, xf, [(r * 1.22, cz - r * 0.22), (r * 1.22, cz + r * 0.3), (0.0, cz + r * 0.3)], **kw)
    for u in (-0.18, 0.0, 0.18):
        box(b, (u - 0.05, -0.03, cz - r * 1.06 - 0.05), (u + 0.05, 0.0, cz - r * 1.06 + 0.05), "amber",
            skip="+y", xf=xf, name="Pane")
    # The shoulders: steps falling from the spine's foot to the edges, and
    # a rule down from each.
    zs = h * 0.36
    polyline(b, xf, [(e, zs), (w * 0.36, zs), (w * 0.36, zs + 0.06 * h), (w * 0.24, zs + 0.06 * h),
                     (w * 0.24, spine), (0.0, spine)], **kw)
    iw, ih = inset
    edge = iw / 2 + 0.36 if iw else 0.0
    polyline(b, xf, [(w * 0.36, zs), (w * 0.36, h * 0.2)], **kw)
    for z in (h * 0.2, h * 0.12):
        polyline(b, xf, [(e, z), (edge, z)], **kw)
    if iw:
        polyline(b, xf, [(iw / 2 + 0.18, 0.12), (iw / 2 + 0.18, ih + 0.62), (iw * 0.22, ih + 0.62),
                         (iw * 0.22, ih + 0.84), (0.0, ih + 0.84)], **kw)


def portal(b, x, y_face, z0, w=5.0, h=7.2, recess=1.0, depth=1.6, iw=2.2, ih=2.8):
    """The very tall circuit-relief portal: a bronze frame on the face at
    `y_face`, a deep reveal, the copper leaf `recess` m in carrying the
    seal in dark bronze, and the small inset door at its foot standing
    open; its passage runs through the wall `depth` m deep."""
    fw = 0.32
    box(b, (x - w / 2 - fw, y_face - 0.1, z0), (x - w / 2, y_face, z0 + h), "bronze", skip="-z +y",
        name="PortalFrame")
    box(b, (x + w / 2, y_face - 0.1, z0), (x + w / 2 + fw, y_face, z0 + h), "bronze", skip="-z +y",
        name="PortalFrame")
    box(b, (x - w / 2 - fw, y_face - 0.1, z0 + h), (x + w / 2 + fw, y_face, z0 + h + fw), "bronze", skip="+y",
        name="PortalFrame")
    yl, yb = y_face + recess, y_face + depth
    # The leaf, round the inset's opening and through to the inner face.
    box(b, (x - w / 2, yl, z0), (x - iw / 2, yb, z0 + h), "copper", skip="-z +y -x", name="Leaf")
    box(b, (x + iw / 2, yl, z0), (x + w / 2, yb, z0 + h), "copper", skip="-z +y +x", name="Leaf")
    box(b, (x - iw / 2, yl, z0 + ih), (x + iw / 2, yb, z0 + h), "copper", skip="+z +y -x +x", name="Leaf")
    # An inner step of the frame round the leaf.
    for lo, hi in (((x - w / 2, yl - 0.12, z0), (x - w / 2 + 0.22, yl, z0 + h)),
                   ((x + w / 2 - 0.22, yl - 0.12, z0), (x + w / 2, yl, z0 + h)),
                   ((x - w / 2 + 0.22, yl - 0.12, z0 + h - 0.22), (x + w / 2 - 0.22, yl, z0 + h))):
        box(b, lo, hi, "bronze", skip="+y -z", name="PortalFrame")
    seal(b, local((x, yl, z0), 0.0), w - 0.44, h - 0.22, inset=(iw, ih))
    # The inset door's frame: jambs and a heavy stepped lintel.
    box(b, (x - iw / 2 - 0.2, yl - 0.1, z0), (x - iw / 2, yl, z0 + ih), "bronze", skip="-z +y", name="InsetFrame")
    box(b, (x + iw / 2, yl - 0.1, z0), (x + iw / 2 + 0.2, yl, z0 + ih), "bronze", skip="-z +y", name="InsetFrame")
    box(b, (x - iw / 2 - 0.36, yl - 0.16, z0 + ih), (x + iw / 2 + 0.36, yl, z0 + ih + 0.3), "bronze", skip="+y",
        name="InsetLintel")
    box(b, (x - iw / 2 - 0.12, yl - 0.1, z0 + ih + 0.3), (x + iw / 2 + 0.12, yl, z0 + ih + 0.46), "bronze",
        skip="+y -z", name="InsetLintel")
    # Its two leaves, swung in against the passage's sides.
    leaf_w = iw / 2 - 0.02
    for s in (-1, 1):
        hinge = Vector((x + s * iw / 2, yl + 0.08, z0))
        xf = (Matrix.Translation(hinge) @ Matrix.Rotation(math.radians(-90 * s), 4, "Z")
              @ Matrix.Translation(Vector((-s * leaf_w / 2, -0.06, 0.0))))
        box(b, (-leaf_w / 2, 0.0, 0.0), (leaf_w / 2, 0.06, ih - 0.04), "copper", skip="-z", xf=xf,
            name="InsetLeaf")
        if not FAR:
            polyline(b, xf, [(-leaf_w / 2 + 0.12, ih * 0.5), (leaf_w / 2 - 0.12, ih * 0.5)], mirror=False,
                     width=0.04, proud=0.012)


def bronze_column(b, x, y, z0, z1, d=0.85, plinth=1.35, sides=12):
    """A smooth column on a high square plinth, banded in bronze at its
    foot and under its plain square capital."""
    r = d / 2
    p = r + 0.32
    box(b, (x - p, y - p, z0), (x + p, y + p, z0 + plinth - 0.14), "lime", skip="-z +z", name="Plinth")
    box(b, (x - p - 0.06, y - p - 0.06, z0 + plinth - 0.14), (x + p + 0.06, y + p + 0.06, z0 + plinth), "shade",
        name="PlinthCap")
    zb = z0 + plinth
    n = 6 if FAR else sides
    prism(b, (x, y), r + 0.035, zb, zb + 1.1, n, "bronze", name="BronzeBase", cap=True)
    prism(b, (x, y), r, zb + 1.1, z1 - 0.9, n, "lime")
    prism(b, (x, y), r + 0.035, z1 - 0.9, z1 - 0.45, n, "bronze", name="BronzeNeck")
    box(b, (x - r - 0.08, y - r - 0.08, z1 - 0.45), (x + r + 0.08, y + r + 0.08, z1 - 0.22), "lime",
        name="Echinus")
    box(b, (x - r - 0.22, y - r - 0.22, z1 - 0.22), (x + r + 0.22, y + r + 0.22, z1), "lime", skip="+z",
        name="Abacus")
    b.collide("column", (x - p - 0.06, y - p - 0.06, z0), (x + p + 0.06, y + p + 0.06, z1))


def side_frame(side, at):
    """The local frame of a face of a plan: `side` is "-y", "+y", "-x", or
    "+x", the face at y or x = `at`; u runs along it, left to right seen
    from outside, and -y points out of it."""
    rot = {"-y": 0.0, "+y": 180.0, "-x": -90.0, "+x": 90.0}[side]
    origin = (0, at, 0) if side in ("-y", "+y") else (at, 0, 0)
    return local(origin, rot)


def side_span(side, x0, x1, y0, y1):
    """The u range a face of the plan from (x0, y0) to (x1, y1) covers in
    `side_frame`."""
    return {"-y": (x0, x1), "+y": (-x1, -x0), "-x": (-y1, -y0), "+x": (y0, y1)}[side]


def dentil_cornice(b, x0, x1, y0, y1, z, frieze=0.8, slab=0.65, out=1.0, sides=("-y", "-x", "+x"),
                   dentil_to=None):
    """A deep, heavy flat cornice over a plan: a plain frieze band with a
    row of dentils under its top, a shade fillet, and a thick flat slab
    overhanging `out` on every side. Dentils run along `sides`; on the x
    faces only as far back as `dentil_to`."""
    zt = z + frieze
    box(b, (x0, y0, z), (x1, y1, zt), "lime", skip="-z +z", name="Frieze")
    if not FAR:
        for side in sides:
            ya, yb = y0, y1 if dentil_to is None else min(y1, dentil_to)
            at = {"-y": y0, "+y": y1, "-x": x0, "+x": x1}[side]
            u0, u1 = side_span(side, x0, x1, ya, yb)
            dentils(b, side_frame(side, at), u0 + 0.12, u1 - 0.12, zt - 0.1, tooth=0.18, gap=0.16, out=0.2,
                    tall=0.26, mat="lime")
        box(b, (x0 - 0.3, y0 - 0.3, zt - 0.1), (x1 + 0.3, y1 + 0.3, zt), "shade", skip="+z", name="Fillet")
    box(b, (x0 - out, y0 - out, zt), (x1 + out, y1 + out, zt + slab), "lime", name="CorniceSlab")
    return zt + slab


def attic(b, x0, x1, y0, y1, z, h=1.0, cap=0.3, over=0.25):
    """A plain stepped attic block set back on a cornice, under a thin
    overhanging cap slab."""
    box(b, (x0, y0, z), (x1, y1, z + h), "lime", skip="-z +z", name="Attic")
    box(b, (x0 - over, y0 - over, z + h), (x1 + over, y1 + over, z + h + cap), "lime", skip="-z" if FAR else "",
        name="AtticCap")
    return z + h + cap


def tall_window(b, xf, u, z0, w=1.0, h=3.0):
    """A tall narrow dark window on a wall's face, in a bronze frame with a
    mullion and two transoms, over a stone sill and under a plain head."""
    box(b, (u - w / 2, -0.02, z0), (u + w / 2, 0.0, z0 + h), "glass", skip=only("-y"), xf=xf, name="Glass")
    if FAR:
        return
    f = 0.08
    for lo, hi in (((u - w / 2 - f, -0.08, z0), (u - w / 2, 0.0, z0 + h)),
                   ((u + w / 2, -0.08, z0), (u + w / 2 + f, 0.0, z0 + h)),
                   ((u - w / 2 - f, -0.08, z0 + h), (u + w / 2 + f, 0.0, z0 + h + f))):
        box(b, lo, hi, "bronze", skip="+y -z", xf=xf, name="WindowFrame")
    box(b, (u - 0.025, -0.05, z0), (u + 0.025, 0.0, z0 + h), "bronze", skip="+y -z +z", xf=xf, name="Mullion")
    for k in (1, 2):
        zk = z0 + h * k / 3
        box(b, (u - w / 2, -0.05, zk - 0.025), (u + w / 2, 0.0, zk + 0.025), "bronze", skip="+y -x +x", xf=xf,
            name="Mullion")
    box(b, (u - w / 2 - 0.14, -0.14, z0 - 0.12), (u + w / 2 + 0.14, 0.0, z0), "shade", skip="+y", xf=xf,
        name="Sill")
    box(b, (u - w / 2 - 0.12, -0.1, z0 + h + f), (u + w / 2 + 0.12, 0.0, z0 + h + f + 0.16), "lime", skip="+y",
        xf=xf, name="Head")


def paired_windows(b, xf, u, z0, w=1.0, h=3.0, gap=1.0):
    """Two tall narrow windows side by side, `gap` m apart."""
    for s in (-1, 1):
        tall_window(b, xf, u + s * (w + gap) / 2, z0, w, h)


def bowl(b, x, y, z, r=0.7, h=0.3):
    """A shallow bronze bowl on a short foot, planted with a low shrub."""
    n = 6 if FAR else 10
    b.solid("Bowl", bl.frustum_mesh("Bowl", (x, y), r * 0.3, r * 0.26, z, z + 0.14, 6, b.mats["bronze"],
                                    cap=False))
    b.solid("Bowl", bl.frustum_mesh("Bowl", (x, y), r * 0.42, r, z + 0.14, z + 0.14 + h, n, b.mats["bronze"]))
    if not FAR:
        sphere(b, (x, y, z + 0.14 + h), r * 0.8, "hedge", segments=8, rings=4, squash=0.42, name="Shrub")


def bowl_wall(b, x0, x1, y0, y1, z0, top, hedge=0.0):
    """A low dark walnut planter wall under a bronze coping, with a low
    clipped hedge along it."""
    box(b, (x0, y0, z0), (x1, y1, top - 0.06), "walnut", skip="-z +z", name="BowlWall")
    box(b, (x0 - 0.04, y0 - 0.04, top - 0.06), (x1 + 0.04, y1 + 0.04, top), "bronze", skip="-z", name="Coping")
    if hedge > 0:
        box(b, (x0 + 0.12, y0 + 0.12, top), (x1 - 0.12, y1 - 0.12, top + hedge), "hedge", skip="-z", name="Hedge")
    b.collide("planter", (x0, y0, z0), (x1, y1, top + hedge))


def sector(b, c, r0, r1, a0, a1, z0, z1, mat, faces=("top", "in"), name="Tier"):
    """A ring sector round `c` between radii `r0` and `r1` and angles `a0`
    and `a1` (radians, from +x toward +y), from `z0` to `z1`, with only
    `faces` drawn: top, in (toward the center), out, a0, and a1 (its
    ends)."""
    cx, cy = c

    def p(r, a, z):
        return Vector((cx + r * math.cos(a), cy + r * math.sin(a), z))

    def radial(a):
        return Vector((math.cos(a), math.sin(a), 0.0))

    am = (a0 + a1) / 2
    quads = []
    if "top" in faces:
        quads.append(([p(r0, a0, z1), p(r0, a1, z1), p(r1, a1, z1), p(r1, a0, z1)], Vector((0, 0, 1))))
    if "in" in faces:
        quads.append(([p(r0, a0, z0), p(r0, a1, z0), p(r0, a1, z1), p(r0, a0, z1)], -radial(am)))
    if "out" in faces:
        quads.append(([p(r1, a0, z0), p(r1, a1, z0), p(r1, a1, z1), p(r1, a0, z1)], radial(am)))
    if "a0" in faces:
        quads.append(([p(r0, a0, z0), p(r1, a0, z0), p(r1, a0, z1), p(r0, a0, z1)], -radial(a0 + math.pi / 2)))
    if "a1" in faces:
        quads.append(([p(r0, a1, z0), p(r1, a1, z0), p(r1, a1, z1), p(r0, a1, z1)], radial(a1 + math.pi / 2)))
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    for q, want in quads:
        f = bm.faces.new([bm.verts.new(v) for v in q])
        f.normal_update()
        if f.normal.dot(want) < 0:
            f.normal_flip()
        for loop in f.loops:
            co = loop.vert.co
            loop[uv].uv = (co.x / 2, co.y / 2) if want.z > 0.5 else ((co.x + co.y) / 2, co.z / 2)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(b.mats[mat])
    return b.solid(name, mesh)


def council_ring(b, c, z0, radii=(3.6, 4.8, 6.0, 7.2), rise=0.32, seg=15.0, gap=37.5):
    """Tiered benches in a ring round a central floor, open in two aisles
    along y, each `gap` degrees either side of its axis: stone tiers a step
    up each, a walnut bench along each tier's back with a copper nosing at
    its front, and a walnut wall behind the last."""
    n = round(360 / seg)
    off = seg / 2

    def open_at(i):
        mid = off + seg * (i + 0.5)
        return any(abs((mid - g + 180) % 360 - 180) < gap for g in (90.0, 270.0))

    for i in range(n):
        if open_at(i):
            continue
        a0, a1 = math.radians(off + seg * i), math.radians(off + seg * (i + 1))
        ends = tuple(e for e, j in (("a0", i - 1), ("a1", i + 1)) if open_at(j % n))
        for k in range(len(radii) - 1):
            r0, r1 = radii[k], radii[k + 1]
            top = z0 + rise * (k + 1)
            sector(b, c, r0, r1, a0, a1, z0, top, "shade", ("top", "in") + ends)
            if FAR:
                continue
            sector(b, c, r1 - 0.55, r1 - 0.08, a0, a1, top, top + 0.42, "walnut", ("top", "in") + ends, name="Bench")
            sector(b, c, r0, r0 + 0.06, a0, a1, top, top + 0.004, "copper", ("top",), name="Nosing")
        last = radii[-1]
        sector(b, c, last, last + 0.14, a0, a1, z0, z0 + rise * (len(radii) - 1) + 0.9, "walnut",
               ("top", "in", "out") + ends, name="BenchWall")
    # Navigation's boxes: three for each half of the ring.
    cx, cy = c
    r0, r1 = radii[0], radii[-1] + 0.14
    top = z0 + rise * (len(radii) - 1) + 0.9
    lo, hi = math.radians(90.0 - gap), math.radians(17.5)
    for s in (-1, 1):
        xa, xb = sorted((s * r0 * math.cos(hi), s * r1))
        b.collide("tiers", (cx + xa, cy - r1 * math.sin(hi), z0), (cx + xb, cy + r1 * math.sin(hi), top))
        for t in (-1, 1):
            xa, xb = sorted((s * r0 * math.cos(lo), s * r1 * math.cos(hi)))
            ya, yb = sorted((t * r0 * math.sin(hi), t * r1 * math.sin(lo)))
            b.collide("tiers", (cx + xa, cy + ya, z0), (cx + xb, cy + yb, top))


def dais(b, x, y0, y1, z0, w=4.0):
    """The speaker's dais: two broad steps, stone and marble, with copper
    nosings."""
    box(b, (x - w / 2, y0, z0), (x + w / 2, y1, z0 + 0.3), "shade", skip="-z", name="Dais")
    box(b, (x - w / 2 + 0.5, y0 + 0.5, z0 + 0.3), (x + w / 2 - 0.5, y1, z0 + 0.6), "marble", skip="-z", name="Dais")
    if not FAR:
        for (xa, xb, ya, z) in ((x - w / 2, x + w / 2, y0, z0 + 0.3), (x - w / 2 + 0.5, x + w / 2 - 0.5, y0 + 0.5,
                                                                         z0 + 0.6)):
            box(b, (xa, ya, z), (xb, ya + 0.06, z + 0.004), "copper", skip=only("+z"), name="Nosing")


def high_chair(b, x, y, z0):
    """The speaker's high-backed walnut chair with a red-brown seat,
    facing -y."""
    box(b, (x - 0.36, y - 0.3, 0.0 + z0), (x + 0.36, y + 0.3, z0 + 0.46), "walnut", skip="-z", name="Chair")
    box(b, (x - 0.32, y - 0.28, z0 + 0.46), (x + 0.32, y + 0.2, z0 + 0.54), "redbrown", skip="-z", name="Chair")
    box(b, (x - 0.4, y + 0.2, z0), (x + 0.4, y + 0.34, z0 + 1.95), "walnut", skip="-z", name="ChairBack")
    for s in (-1, 1):
        box(b, (x + s * 0.4 - 0.06, y - 0.3, z0 + 0.46), (x + s * 0.4 + 0.06, y + 0.2, z0 + 0.78), "walnut",
            skip="-z", name="ChairArm")
    if not FAR:
        box(b, (x - 0.4, y + 0.19, z0 + 1.7), (x + 0.4, y + 0.2, z0 + 1.76), "copper", skip=only("-y"),
            name="ChairBand")
    b.collide("chair", (x - 0.46, y - 0.3, z0), (x + 0.46, y + 0.34, z0 + 1.95))


def seal_panel(b, xf, w, h, z0):
    """The seal on a copper plate in a white marble surround on a wall."""
    box(b, (-w / 2, -0.06, z0), (w / 2, 0.0, z0 + h), "copper", skip="+y", xf=xf, name="SealPlate")
    m = 0.32
    for lo, hi in (((-w / 2 - m, -0.12, z0 - m), (-w / 2, 0.0, z0 + h + m)),
                   ((w / 2, -0.12, z0 - m), (w / 2 + m, 0.0, z0 + h + m)),
                   ((-w / 2, -0.12, z0 - m), (w / 2, 0.0, z0)),
                   ((-w / 2, -0.12, z0 + h), (w / 2, 0.0, z0 + h + m))):
        box(b, lo, hi, "marble", skip="+y", xf=xf, name="SealSurround")
    seal(b, xf @ Matrix.Translation(Vector((0.0, -0.06, z0))), w, h)


# The Civic Hall: heights above the ground, m. The reference image's
# building is broad and low: two storeys of about 4.6 m on a podium.
C_FLOOR = 1.92  # The podium: twelve steps of 0.16 m.
C_PLINTH = 1.35  # The columns' high plinths.
C_ARCH = C_FLOOR + 8.4  # The columns' tops, and the portico's beam.
C_WALL = C_FLOOR + 9.3  # The walls' tops.
C_CEIL = C_FLOOR + 8.6  # The chamber's ceiling, under the roof.
# Plan, m (Blender y grows away from the street).
C_HW = 18.0  # The wings' outer faces.
C_PAV = 8.6  # The pavilion's half width.
C_PAV_Y = 8.4  # The pavilion's face.
C_WING_Y = 9.6  # The wings' faces: the pavilion projects 1.2 m.
C_HALL_Y = 10.0  # The chamber's front wall, inside.
C_BACK = 30.0  # The back wall, inside.
C_HALL_HW = 10.6  # The chamber's half width.
C_COL_Y = 6.4  # The columns' line.
C_RING = (0.0, 19.6)  # The council ring's center.


def civic_body(b):
    """The Civic Hall's outside: the stair between dark planter walls with
    bronze bowls, the podium, four columns on high plinths before the
    pavilion, the portal, the wings with their paired windows, and the
    crown: the dentil cornices, the pavilion's heavy slab, and its attic."""
    f, hw = C_FLOOR, C_HW
    # -- The approach: twelve shallow steps between stepped walnut walls.
    y_top, _ = stair(b, -7.2, 7.2, 0.0, 0.0, 12, f / 12, 0.4)
    for s in (-1, 1):
        x0, x1 = sorted((s * 7.2, s * 9.6))
        bowl_wall(b, x0, x1, 0.0, 2.4, 0.0, 0.75)
        bowl_wall(b, x0, x1, 2.4, y_top, 0.0, 1.55)
        bowl(b, s * 8.4, 1.0, 0.75, r=0.75)
        bowl(b, s * 8.4, 3.6, 1.55, r=0.75)
        # Long planters along the podium's foot, hedged, a bowl at the end.
        x0, x1 = sorted((s * 9.6, s * 16.6))
        bowl_wall(b, x0, x1, y_top - 1.7, y_top, 0.0, 0.85, hedge=0.4 if not FAR else 0.3)
        bowl(b, s * 17.4, y_top - 0.85, 0.0, r=0.7)
    # -- The podium, with a darker base course.
    box(b, (-hw - 0.4, y_top, 0.0), (hw + 0.4, C_BACK + 0.8, f), "lime", skip="-z", name="Podium")
    box(b, (-hw - 0.48, y_top - 0.08, 0.0), (hw + 0.48, C_BACK + 0.88, 0.3), "shade", skip="-z",
        name="BaseCourse")
    # -- The portico: four columns before the pavilion, under its beam.
    for x in (-6.9, -2.9, 2.9, 6.9):
        bronze_column(b, x, C_COL_Y, f, C_ARCH)
    beam_y = C_COL_Y - 0.8
    box(b, (-C_PAV, beam_y, C_ARCH), (C_PAV, C_PAV_Y, C_WALL), "lime", skip="+z +y", name="Architrave")
    # -- The pavilion's face round the portal, and the portal.
    door_w, door_h = 5.0, 7.2
    for s in (-1, 1):
        x0, x1 = sorted((s * door_w / 2, s * C_PAV))
        box(b, (x0, C_PAV_Y, f), (x1, C_HALL_Y, C_WALL), "lime", skip="-z +z", name="Wall")
        b.collide("facade", (x0, C_PAV_Y, f), (x1, C_HALL_Y, C_WALL))
    box(b, (-door_w / 2, C_PAV_Y, f + door_h), (door_w / 2, C_HALL_Y, C_WALL), "lime", skip="+z -x +x",
        name="Wall")
    portal(b, 0.0, C_PAV_Y, f, w=door_w, h=door_h)
    # The chamber's side of the portal: plain wall round the inset door.
    if not FAR:
        for lo, hi in (((-door_w / 2, C_HALL_Y, f), (-1.1, C_HALL_Y + 0.01, f + door_h)),
                       ((1.1, C_HALL_Y, f), (door_w / 2, C_HALL_Y + 0.01, f + door_h)),
                       ((-1.1, C_HALL_Y, f + 2.8), (1.1, C_HALL_Y + 0.01, f + door_h))):
            box(b, lo, hi, "lime", skip=only("+y"), name="Wall")
    # -- The wings: front walls beside the pavilion, side walls, the back
    # wall, and the chamber's side walls between the wings and the hall.
    for s in (-1, 1):
        x0, x1 = sorted((s * C_PAV, s * (hw - 0.4)))
        box(b, (x0, C_WING_Y, f), (x1, C_HALL_Y, C_WALL), "lime", skip="-z +z -x +x", name="Wall")
        b.collide("wing", (x0, C_WING_Y, f), (x1, C_HALL_Y, C_WALL))
        x0, x1 = sorted((s * (hw - 0.4), s * hw))
        box(b, (x0, C_WING_Y, f), (x1, C_BACK + 0.4, C_WALL), "lime",
            skip="-z +z +y " + ("-x" if s > 0 else "+x"), name="Wall")
        b.collide("wall", (x0, C_WING_Y, f), (x1, C_BACK + 0.4, C_WALL))
        x0, x1 = sorted((s * C_HALL_HW, s * (C_HALL_HW + 0.4)))
        box(b, (x0, C_HALL_Y, f), (x1, C_BACK, C_WALL), "lime", skip="-z +z -y +y " + ("+x" if s > 0 else "-x"),
            name="Wall")
        b.collide("wall", (x0, C_HALL_Y, f), (x1, C_BACK, C_WALL))
    box(b, (-hw + 0.4, C_BACK, f), (hw - 0.4, C_BACK + 0.4, C_WALL), "lime", skip="-z +z -x +x", name="Wall")
    b.collide("back", (-hw, C_BACK, f), (hw, C_BACK + 0.4, C_WALL))
    # The paired windows: on each wing's front and side, both storeys.
    front = local((0, C_WING_Y, 0), 0.0)
    for s in (-1, 1):
        side = side_frame("+x" if s > 0 else "-x", s * hw)
        for xf, u in ((front, s * 13.1), (side, 15.5 * s), (side, 24.5 * s)):
            paired_windows(b, xf, u, f + 0.9, h=3.1)
            paired_windows(b, xf, u, f + 5.1, h=2.7)
    # A thin band under the cornice, round the wings.
    if not FAR:
        for s in (-1, 1):
            x0, x1 = sorted((s * C_PAV, s * hw))
            box(b, (x0, C_WING_Y - 0.08, f + 8.15), (x1, C_WING_Y, f + 8.35), "shade", skip="+y -x", name="Band")
            x0, x1 = sorted((s * hw, s * (hw + 0.08)))
            box(b, (x0, C_WING_Y - 0.08, f + 8.15), (x1, C_BACK + 0.4, f + 8.35), "shade", skip="-y +y",
                name="Band")
    # -- The crown. The wings' cornice: dentils under a slab overhanging
    # the walls, then a plain parapet block, the walkable roof.
    if not FAR:
        for s in (-1, 1):
            x0, x1 = sorted((s * C_PAV, s * hw))
            dentils(b, front, x0 + 0.12, x1 - 0.12, C_WALL, tooth=0.16, gap=0.16, out=0.16, tall=0.22,
                    mat="lime")
            u0, u1 = side_span("+x" if s > 0 else "-x", -hw, hw, C_WING_Y, C_BACK + 0.4)
            dentils(b, side_frame("+x" if s > 0 else "-x", s * hw), u0 + 0.12, u1 - 0.12, C_WALL, tooth=0.16,
                    gap=0.16, out=0.16, tall=0.22, mat="lime")
    box(b, (-hw - 0.5, C_WING_Y - 0.5, C_WALL), (hw + 0.5, C_BACK + 0.9, C_WALL + 0.45), "lime",
        name="CorniceSlab")
    roof = C_WALL + 1.2
    box(b, (-hw + 0.3, C_WING_Y + 0.3, C_WALL + 0.45), (hw - 0.3, C_BACK + 0.1, roof - 0.15), "lime",
        skip="-z +z", name="Parapet")
    box(b, (-hw + 0.2, C_WING_Y + 0.2, roof - 0.15), (hw - 0.2, C_BACK + 0.2, roof), "shade", skip="-z",
        name="ParapetCap")
    b.roofs.append(((0.0, (C_WING_Y + C_BACK + 0.4) / 2), True, ((C_BACK - C_WING_Y) / 2, hw - 0.2), roof,
                    roof + 0.01))
    # The pavilion's crown: a frieze with dentils over the portico's beam,
    # the heavy slab, and the stepped attic.
    crown_y = 14.0
    slab_top = dentil_cornice(b, -C_PAV, C_PAV, beam_y, crown_y, C_WALL, frieze=0.8, slab=0.65, out=1.0,
                              dentil_to=C_WING_Y)
    attic_top = attic(b, -7.8, 7.8, beam_y + 0.4, crown_y - 0.4, slab_top, h=1.0, cap=0.3, over=0.25)
    b.roofs.append(((0.0, (beam_y + crown_y) / 2), True, ((crown_y - beam_y) / 2 - 0.15, 8.05), attic_top,
                    attic_top + 0.01))
    # -- Light: lanterns on posts at the stair's foot, uplights washing the
    # portal, and lanterns on the wings' faces beside the pavilion.
    for s in (-1, 1):
        lantern_post(b, s * 8.4, -0.6, 0.0)
        uplight(b, s * 3.6, C_PAV_Y - 0.55, f, toward=(0.0, 1.0))
        wall_lantern(b, s * 10.2, C_WING_Y, f + 3.2)
    b.front = (0.0, -1.0)


def civic_chamber(b):
    """The council chamber: the ring of tiered benches round a well with a
    copper seal inlaid in its floor, the speaker's dais and lectern under
    the seal on the back wall, braziers, a coffered ceiling with copper
    inlays, pilasters, walnut wainscot inscribed with circuit lines, and
    candlelight."""
    b.inside = C_RING
    if FAR:
        return
    f = C_FLOOR
    x0, x1, y0, y1 = -C_HALL_HW, C_HALL_HW, C_HALL_Y, C_BACK
    slab(b, x0, x1, y0, y1, f + 0.02, "marble", name="Floor")
    slab(b, x0, x1, y0, y1, C_CEIL, "shade", down=True, name="Ceiling")
    xs = [-7.95 + 2.65 * i for i in range(7)]
    ys = [y0 + (y1 - y0) * (k + 1) / 7 for k in range(6)]
    coffers(b, x0, x1, y0, y1, C_CEIL, xs=xs, ys=ys)
    ex, ey = [x0 + 0.55] + xs + [x1 - 0.55], [y0 + 0.55] + ys + [y1 - 0.55]
    for xa, xb in zip(ex, ex[1:]):
        for ya, yb in zip(ey, ey[1:]):
            cx, cy = (xa + xb) / 2, (ya + yb) / 2
            box(b, (cx - 0.28, cy - 0.28, C_CEIL - 0.012), (cx + 0.28, cy + 0.28, C_CEIL - 0.002), "copper",
                skip=only("-z"), name="CofferInlay")
    cove_light(b, x0, x1, y0, y1, C_CEIL - 0.285)
    # The well: the seal's circle and cross inlaid in copper in the floor.
    cx, cy = C_RING
    flat = Matrix.Translation(Vector((0.0, 0.0, f + 0.02))) @ FLAT
    kw = {"mat": "copper", "width": 0.07, "proud": 0.006}
    for r in (2.6, 0.7):
        arc(b, flat, (cx, cy), r, 0.0, 2 * math.pi, 24 if r > 1 else 10, **kw)
    arc(b, flat, (cx, cy), 3.3, 0.0, 2 * math.pi, 28, **kw)
    trace(b, flat, (cx - 3.3, cy), (cx + 3.3, cy), **kw)
    trace(b, flat, (cx, cy - 3.3), (cx, cy + 3.3), **kw)
    council_ring(b, C_RING, f + 0.02)
    # The speaker's dais in the ring's back aisle, the lectern on it facing
    # the room, and the speaker's chair behind.
    dais(b, 0.0, cy + 4.0, cy + 8.0, f + 0.02)
    lectern(b, 0.0, cy + 5.3, f + 0.62)
    candelabra(b, 0.0, cy + 5.3, f + 0.62 + 1.06, arms=0.15, stem=0.2)
    high_chair(b, 0.0, cy + 7.1, f + 0.62)
    # The back wall: the seal on a copper plate, pilasters, and braziers.
    back = local((0, y1, 0), 0.0)
    seal_panel(b, back, 4.2, 5.6, f + 1.6)
    for u in (-3.7, 3.7, -8.8, 8.8):
        pilaster(b, back, u, f, C_CEIL - 0.5)
    for s in (-1, 1):
        brazier(b, s * 3.3, y1 - 1.5, f + 0.02)
    # The side walls: pilasters, walnut wainscot with faint circuit lines,
    # and a sconce over each panel.
    for s in (-1, 1):
        side = local((s * C_HALL_HW, 0, 0), -90 * s)
        for y in (13.0, 20.0, 27.0):
            pilaster(b, side, -y * s, f, C_CEIL - 0.5)
        for ya, yb in ((13.5, 19.5), (20.5, 26.5)):
            u0, u1 = sorted((-ya * s, -yb * s))
            uc = (u0 + u1) / 2
            box(b, (u0 + 0.1, -0.06, f), (u1 - 0.1, 0.0, f + 2.6), "walnut", skip="+y -z", xf=side,
                name="Wainscot")
            circuit_lines(b, side @ Matrix.Translation(Vector((uc, -0.06, 0.0))), u1 - u0 - 0.4, f + 0.2, 2.3)
            sconce(b, side, uc, f + 3.4)
    # The front wall: sconces flanking the inset door, and tall candle
    # stands in the four corners.
    door_face = local((0, y0, 0), 180.0)
    for u in (-2.4, 2.4):
        sconce(b, door_face, u, f + 2.6)
    for x in (-9.5, 9.5):
        for y in (11.2, 28.8):
            floor_candelabrum(b, x, y, f + 0.02)


# --------------------------------------------------------------------------
# The belvedere's kit, from the sixth and seventh reference images: a
# loggia that opens through inlaid marble piers onto a terrace with a
# view, under a dark red-brown lintel band inlaid with copper; and an entry
# court whose colonnade frames a mahogany double door in a stepped bronze
# surround, over a floor inlaid with a meander.


def disc(b, xf, u, z, r, mat="copper", proud=0.03, sides=16, name="Disc"):
    """A flat round plate on a face (the local plane y = 0, facing -y),
    centered at (u, z): a medallion or an inlaid disc."""
    mesh = bl.frustum_mesh(name, (0.0, 0.0), r, r, 0.0, proud, sides, b.mats[mat])
    mesh.transform(xf @ Matrix.Translation(Vector((u, 0.0, z))) @ Matrix.Rotation(math.radians(90.0), 4, "X"))
    for p in mesh.polygons:
        p.use_smooth = False
    return b.solid(name, mesh)


def inlaid_pier(b, x, y0, y1, z0, z1, w=1.0, stripes=True):
    """A square white marble pier on a low base, its front and back faces
    inlaid with three thin bronze lines under a short bar, capped by a
    copper corbel block."""
    box(b, (x - w / 2 - 0.06, y0 - 0.06, z0), (x + w / 2 + 0.06, y1 + 0.06, z0 + 0.3), "marble", skip="-z",
        name="PierBase")
    box(b, (x - w / 2, y0, z0 + 0.3), (x + w / 2, y1, z1 - 0.38), "marble", skip="-z +z", name="Pier")
    box(b, (x - w / 2 - 0.1, y0 - 0.1, z1 - 0.38), (x + w / 2 + 0.1, y1 + 0.1, z1), "copper", skip="+z",
        name="Corbel")
    b.collide("pier", (x - w / 2 - 0.1, y0 - 0.1, z0), (x + w / 2 + 0.1, y1 + 0.1, z1))
    if FAR or not stripes:
        return
    for face in (local((x, y0, 0), 0.0), local((x, y1, 0), 180.0)):
        kw = {"mat": "bronze", "width": 0.05, "proud": 0.012}
        za, zb = z0 + 0.75, z1 - 0.85
        for u in (-0.2, 0.0, 0.2):
            trace(b, face, (u, za), (u, zb), **kw)
        trace(b, face, (-0.28, zb + 0.12), (0.28, zb + 0.12), **kw)
        trace(b, face, (-0.28, za - 0.12), (0.28, za - 0.12), **kw)


def lintel_band(b, xf, w, z0, h=1.1):
    """A deep dark red-brown band on a face (the local plane y = 0, facing -y)
    from u = -w/2 to w/2: copper line work of a disc with a line through
    it, groups of vertical bars either side, and frames at the ends."""
    box(b, (-w / 2, -0.05, z0), (w / 2, 0.0, z0 + h), "redbrown", skip="+y", xf=xf, name="LintelBand")
    if FAR:
        return
    face = xf @ Matrix.Translation(Vector((0.0, -0.05, 0.0)))
    kw = {"mat": "copper", "width": 0.04, "proud": 0.012}
    zc, e = z0 + h / 2, w / 2 - 0.1
    polyline(b, face, [(0.0, z0 + 0.1), (e, z0 + 0.1), (e, z0 + h - 0.1), (0.0, z0 + h - 0.1)], **kw)
    r = h * 0.3
    disc(b, face, 0.0, zc, r, proud=0.02)
    arc(b, face, (0.0, zc), r + 0.1, 0.0, 2 * math.pi, 20, **kw)
    # The line through the disc, from bar group to bar group.
    bars = w * 0.18
    polyline(b, face, [(r + 0.1, zc), (bars - 0.2, zc)], **kw)
    for s in (-1, 1):
        for k in range(5):
            u = s * (bars + 0.13 * k)
            trace(b, face, (u, z0 + 0.1), (u, z0 + h - 0.1), **kw)
        # The end frames.
        a, c = s * (w / 2 - 0.35), s * (w / 2 - 0.35 - w * 0.12)
        polyline(b, face, [(a, z0 + 0.24), (a, z0 + h - 0.24), (c, z0 + h - 0.24), (c, z0 + 0.24), (a, z0 + 0.24)],
                 mirror=False, **kw)


def louver(b, xf, u0, u1, z0, z1, slats=9):
    """A dark walnut slat screen high on a wall: horizontal slats in a
    frame before a dark bronze backing."""
    box(b, (u0, -0.02, z0), (u1, 0.0, z1), "bronze", skip=only("-y"), xf=xf, name="LouverBack")
    for lo, hi in (((u0 - 0.08, -0.14, z0 - 0.08), (u0, 0.0, z1 + 0.08)), ((u1, -0.14, z0 - 0.08), (u1 + 0.08, 0.0, z1 + 0.08)),
                   ((u0, -0.14, z0 - 0.08), (u1, 0.0, z0)), ((u0, -0.14, z1), (u1, 0.0, z1 + 0.08))):
        box(b, lo, hi, "walnut", skip="+y", xf=xf, name="LouverFrame")
    if FAR:
        return
    for k in range(slats):
        z = z0 + (z1 - z0) * (k + 0.5) / slats
        box(b, (u0, -0.12, z - 0.04), (u1, -0.03, z + 0.04), "walnut", skip="+y -x +x", xf=xf, name="Slat")


def relief_panel(b, xf, u, z0, w=3.6, h=2.4, figures=4):
    """A copper relief in a walnut frame on a wall: a row of abstract
    standing figures in dark line work, each a round head over a stepped,
    tapering body, on a ground line. Geometric figures of our own, never a
    likeness."""
    box(b, (u - w / 2 - 0.12, -0.06, z0 - 0.12), (u + w / 2 + 0.12, 0.0, z0 + h + 0.12), "walnut", skip="+y", xf=xf,
        name="ReliefFrame")
    box(b, (u - w / 2, -0.1, z0), (u + w / 2, -0.06, z0 + h), "copper", skip="+y", xf=xf, name="Relief")
    if FAR:
        return
    face = xf @ Matrix.Translation(Vector((0.0, -0.1, 0.0)))
    kw = {"mat": "bronze", "width": 0.03, "proud": 0.01}
    trace(b, face, (u - w / 2 + 0.15, z0 + 0.25), (u + w / 2 - 0.15, z0 + 0.25), **kw)
    step = w / figures
    for k in range(figures):
        cx = u - w / 2 + step * (k + 0.5)
        lean = 0.06 * (k % 2 * 2 - 1)
        top = z0 + h * (0.78 + 0.06 * (k % 2))
        arc(b, face, (cx + lean, top + 0.14), 0.12, 0.0, 2 * math.pi, 10, **kw)
        pts = [(cx - 0.32, z0 + 0.25), (cx - 0.22, top - 0.6), (cx - 0.12, top - 0.6), (cx - 0.12, top),
               (cx + 0.12, top), (cx + 0.12, top - 0.6), (cx + 0.22, top - 0.6), (cx + 0.32, z0 + 0.25)]
        polyline(b, face, [(p[0] + lean * (p[1] - z0) / h, p[1]) for p in pts], mirror=False, **kw)
        # An arm, raised on every other figure.
        arm = (cx + 0.12 + lean, top - 0.15), (cx + 0.42 + lean, top - (0.0 if k % 2 else 0.5))
        trace(b, face, *arm, **kw)


def cushioned_bench(b, x0, x1, y0, y1, z0, back="+x"):
    """A low built-in marble bench with dark red cushions on its seat and
    against the wall behind it (`back`, the wall's side)."""
    box(b, (x0, y0, z0), (x1, y1, z0 + 0.32), "marble", skip="-z", name="BenchBase")
    box(b, (x0 + 0.04, y0 + 0.04, z0 + 0.32), (x1 - 0.04, y1 - 0.04, z0 + 0.46), "redbrown", skip="-z",
        name="Cushion")
    b.collide("bench", (x0, y0, z0), (x1, y1, z0 + 0.9))
    if FAR:
        return
    n = max(1, round((y1 - y0) / 0.9) if back in ("+x", "-x") else round((x1 - x0) / 0.9))
    for k in range(n):
        if back in ("+x", "-x"):
            ya, yb = y0 + (y1 - y0) * k / n + 0.04, y0 + (y1 - y0) * (k + 1) / n - 0.04
            xa, xb = (x1 - 0.2, x1) if back == "+x" else (x0, x0 + 0.2)
        else:
            xa, xb = x0 + (x1 - x0) * k / n + 0.04, x0 + (x1 - x0) * (k + 1) / n - 0.04
            ya, yb = (y1 - 0.2, y1) if back == "+y" else (y0, y0 + 0.2)
        box(b, (xa, ya, z0 + 0.46), (xb, yb, z0 + 0.9), "redbrown", skip="-z", name="BackCushion")


def urn_tree(b, x, y, z0, h=2.4, r=0.42):
    """A white stone urn with a small olive tree: a slender trunk under a
    loose crown of three clumps."""
    n = 6 if FAR else 10
    b.solid("Urn", bl.frustum_mesh("Urn", (x, y), r * 0.55, r * 0.5, z0, z0 + 0.15, n, b.mats["marble"],
                                   cap=False))
    b.solid("Urn", bl.frustum_mesh("Urn", (x, y), r * 0.6, r, z0 + 0.15, z0 + 0.7, n, b.mats["marble"]))
    b.collide("urn", (x - r, y - r, z0), (x + r, y + r, z0 + h))
    if FAR:
        box(b, (x - 0.5, y - 0.5, z0 + h * 0.55), (x + 0.5, y + 0.5, z0 + h), "hedge", name="Crown")
        return
    prism(b, (x, y), 0.05, z0 + 0.7, z0 + h * 0.75, 5, "walnut", name="Trunk")
    for dx, dy, dz, rr in ((0.0, 0.0, 0.85, 0.5), (0.28, 0.1, 0.68, 0.36), (-0.25, -0.12, 0.72, 0.34)):
        sphere(b, (x + dx, y + dy, z0 + h * dz), rr, "hedge", segments=7, rings=4, squash=0.75, name="Crown")


def terracotta_pot(b, x, y, z0, r=0.3, shrub=0.7, tree=False):
    """A terracotta pot with a clipped shrub or a small tree."""
    n = 6 if FAR else 9
    b.solid("Pot", bl.frustum_mesh("Pot", (x, y), r * 0.7, r, z0, z0 + r * 1.5, n, b.mats["copper"]))
    if FAR:
        return
    top = z0 + r * 1.5
    if tree:
        prism(b, (x, y), 0.035, top, top + shrub * 0.6, 5, "walnut", name="Trunk")
        sphere(b, (x, y, top + shrub * 0.85), shrub * 0.45, "hedge", segments=7, rings=4, squash=0.85,
               name="Shrub")
    else:
        sphere(b, (x, y, top + shrub * 0.3), shrub * 0.5, "hedge", segments=7, rings=4, squash=0.7, name="Shrub")


def threshold(b, x0, x1, y0, z0, steps=2, rise=0.17, run=0.4):
    """A stepped marble threshold with a fine walnut line along each
    tread's edge."""
    y, z = stair(b, x0, x1, y0, z0, steps, rise, run, mat="marble")
    if not FAR:
        for i in range(steps):
            box(b, (x0, y0 + i * run + 0.05, z0 + rise * (i + 1)), (x1, y0 + i * run + 0.09,
                                                                     z0 + rise * (i + 1) + 0.003),
                "walnut", skip=only("+z"), name="TreadLine")
    return y, z


def side_door(b, xf, u, z0, w=1.3, h=2.8):
    """A heavy walnut door on a wall's face in a marble surround, with a
    curved bronze handle; closed."""
    box(b, (u - w / 2 - 0.18, -0.08, z0), (u + w / 2 + 0.18, 0.0, z0 + h + 0.18), "marble", skip="+y -z", xf=xf,
        name="DoorSurround")
    box(b, (u - w / 2, -0.12, z0), (u + w / 2, -0.08, z0 + h), "walnut", skip="+y -z", xf=xf, name="DoorLeaf")
    if FAR:
        return
    face = xf @ Matrix.Translation(Vector((0.0, -0.12, 0.0)))
    hx = u + w / 2 - 0.22
    arc(b, face, (hx, z0 + 1.05), 0.16, -math.pi / 2, math.pi / 2, 5, mat="bronze", width=0.035, proud=0.06)
    for k in (-1, 1):
        box(b, (hx - 0.04, -0.06, z0 + 1.05 + 0.16 * k - 0.03), (hx + 0.01, 0.0, z0 + 1.05 + 0.16 * k + 0.03),
            "bronze", skip="+y", xf=face, name="Handle")


def mahogany_leaf(b, xf, w, h, pull_left=True):
    """A dark mahogany door leaf on a face (the local plane y = 0, facing
    -y, u from -w/2): fine brass grids near its top and foot, two long
    rails between them, and a round brass medallion pull."""
    box(b, (-w / 2, 0.0, 0.0), (w / 2, 0.08, h), "redbrown", skip="+y -z", xf=xf, name="Mahogany")
    if FAR:
        return
    kw = {"mat": "copper", "width": 0.025, "proud": 0.008}
    a, c = -w / 2 + 0.18, w / 2 - 0.18
    for z0, z1 in ((h - 0.95, h - 0.2), (0.25, 0.85)):
        polyline(b, xf, [(a, z0), (c, z0), (c, z1), (a, z1), (a, z0)], mirror=False, **kw)
        for k in (1, 2, 3):
            u = a + (c - a) * k / 4
            trace(b, xf, (u, z0), (u, z1), **kw)
        for k in (1, 2):
            z = z0 + (z1 - z0) * k / 3
            trace(b, xf, (a, z), (c, z), **kw)
    for u in (a + 0.12, c - 0.12):
        trace(b, xf, (u, 0.85), (u, h - 0.95), **kw)
    pull = (a + 0.18) if pull_left else (c - 0.18)
    disc(b, xf, pull, h * 0.5, 0.15, proud=0.04, sides=14, name="Medallion")


def stepped_surround(b, xf, dw, dh, mat="copper"):
    """The stepped bronze line inlay round a door on a face (the local
    plane y = 0, facing -y, the door's foot at z = 0): nested frames that
    step outward and upward, a filled band in the outer step, and wing
    brackets in the lower corners ending in circles."""
    if FAR:
        return
    kw = {"mat": mat, "width": 0.05, "proud": 0.01}
    frames = [(dw / 2 + 0.25, dh + 0.25), (dw / 2 + 0.7, dh + 0.8), (dw / 2 + 1.2, dh + 1.35), (dw / 2 + 1.75,
                                                                                              dh + 1.95)]
    for k, (e, top) in enumerate(frames):
        foot = 0.0 if k < 2 else dh * 0.62
        polyline(b, xf, [(e, foot), (e, top), (0.0, top)], **kw)
    # The outer step's band, filled, across the top and down the sides.
    (e2, t2), (e3, t3) = frames[2], frames[3]
    band = {"mat": mat, "width": (t3 - t2) * 0.6, "proud": 0.006}
    trace(b, xf, (-(e2 + 0.1), (t2 + t3) / 2), (e2 + 0.1, (t2 + t3) / 2), **band)
    for s in (-1, 1):
        trace(b, xf, (s * (e2 + e3) / 2, dh * 0.68), (s * (e2 + e3) / 2, t3 - 0.3),
              mat=mat, width=(e3 - e2) * 0.6, proud=0.006)
        # The wing: a bar out from the second frame, two diagonals falling
        # outward from it, and a circle at their foot.
        e1 = frames[1][0]
        zb = dh * 0.62
        polyline(b, xf, [(s * e1, zb), (s * e3, zb)], mirror=False, **kw)
        for d in (0.0, 0.32):
            trace(b, xf, (s * (e1 + 0.1 + d), zb), (s * (e3 - 0.05 + d * 0.4), zb - 1.15 + d * 0.3), **kw)
        arc(b, xf, (s * (e3 + 0.05), zb - 1.42), 0.22, 0.0, 2 * math.pi, 12, **kw)
        # A short return under the frames' feet.
        trace(b, xf, (s * (e2 - 0.1), zb + 0.4), (s * (e3 + 0.1), zb + 0.4), **kw)


def meander_floor(b, xf, cx, cy, r=3.0, out=-1.0):
    """A bronze meander and arc inlaid in a floor (`xf` lays the inlay
    flat), before a door at (cx, cy) whose front faces y's `out` sign: a
    half ring round the door's foot and a Greek key across it."""
    if FAR:
        return
    kw = {"mat": "copper", "width": 0.07, "proud": 0.006}
    a = math.pi if out < 0 else 0.0
    arc(b, xf, (cx, cy), r, a, a + math.pi, 16, **kw)
    arc(b, xf, (cx, cy), r - 0.35, a + math.pi * 0.08, a + math.pi * 0.4, 6, **kw)
    arc(b, xf, (cx, cy), r - 0.35, a + math.pi * 0.6, a + math.pi * 0.92, 6, **kw)
    # The key: a run of hooks between two rails.
    y0, y1 = sorted((cy + out * r * 0.55, cy + out * r * 0.3))
    trace(b, xf, (cx - r * 0.8, y0), (cx + r * 0.8, y0), **kw)
    trace(b, xf, (cx - r * 0.8, y1), (cx + r * 0.8, y1), **kw)
    n = 4
    step = r * 1.6 / n
    for k in range(n):
        a = cx - r * 0.8 + step * k + step * 0.2
        h = (y1 - y0)
        polyline(b, xf, [(a, y0), (a, y0 + h * 0.75), (a + step * 0.55, y0 + h * 0.75), (a + step * 0.55,
                                                                                       y0 + h * 0.35),
                         (a + step * 0.3, y0 + h * 0.35)], mirror=False, **kw)


# The belvedere: heights above its origin's ground, m. It stands on rising
# ground, so its back is higher than its front.
V_FLOOR = 3.3  # The terrace: twenty steps of 0.165 m.
V_HALL = V_FLOOR + 0.34  # The loggia and the court, over the threshold.
V_PIER = 4.4  # The piers' height over the loggia's floor.
V_CEIL = 6.0  # The loggia's ceiling over its floor.
# Plan, m (Blender y grows away from the view).
V_TERRACE = 8.0  # The terrace's front edge, at the stair's head.
V_SILL = 16.0  # The threshold's foot.
V_PIERS = (16.8, 17.8)  # The pier line's front and back.
V_BACK = 25.4  # The loggia's back wall, inside.
V_COURT = 26.0  # The portal wall's face, on the court.
V_COLS = 32.6  # The court's colonnade.
V_END = 33.4  # The court's back edge.
V_HW = 7.6  # The loggia's half width outside.
V_TW = 9.5  # The terrace's and the side walks' half width.


def belvedere_body(b):
    """The belvedere's outside: the stair up from the trail, the terrace
    with its parapets, benches, and urn trees, the threshold, the loggia's
    piers, walls, and roof, the side walks, and the entry court with its
    colonnade, the portal, and the meander."""
    f, hall, tw, hw = V_FLOOR, V_HALL, V_TW, V_HW
    # -- The stair from the trail, and the terrace on its retaining wall.
    y_top, _ = stair(b, -2.6, 2.6, 0.0, 0.0, 20, f / 20, 0.4, mat="marble")
    box(b, (-tw, y_top, -3.0), (tw, V_SILL + 0.8, f), "lime", skip="-z", name="Podium")
    box(b, (-tw, V_SILL + 0.8, -3.0), (tw, V_END, hall), "lime", skip="-z", name="Podium")
    for s in (-1, 1):
        x0, x1 = sorted((s * 2.6, s * 3.0))
        for k in range(4):
            ya, yb = y_top * k / 4, y_top * (k + 1) / 4
            box(b, (x0, ya, -3.0), (x1, yb, f * (k + 1) / 4 + 0.5), "marble", skip="-z", name="Cheek")
        b.collide("cheek", (x0, 0.0, 0.0), (x1, y_top, f + 0.5))
        # The parapets along the terrace's front and sides.
        x0, x1 = sorted((s * 3.0, s * tw))
        box(b, (x0, y_top, f), (x1, y_top + 0.4, f + 0.95), "marble", skip="-z", name="Parapet")
        b.collide("parapet", (x0, y_top, f), (x1, y_top + 0.4, f + 0.95))
        x0, x1 = sorted((s * (tw - 0.4), s * tw))
        box(b, (x0, y_top + 0.4, f), (x1, V_SILL, f + 0.95), "marble", skip="-z", name="Parapet")
        box(b, (x0, V_SILL, hall), (x1, V_END - 0.6, hall + 0.95), "marble", skip="-z", name="Parapet")
        b.collide("parapet", (x0, y_top + 0.4, f), (x1, V_END - 0.6, hall + 0.95))
        # Benches facing the view, and urn trees at the corners.
        x0, x1 = sorted((s * 5.7, s * 8.9))
        cushioned_bench(b, x0, x1, y_top + 1.4, y_top + 2.1, f, back="+y")
        urn_tree(b, s * 8.6, y_top + 0.85, f)
        urn_tree(b, s * 3.6, V_SILL - 0.8, f, h=2.2)
        lantern_post(b, s * 3.5, y_top + 0.75, f, height=1.6)
    # -- The threshold, two steps across the terrace's whole width.
    threshold(b, -tw, tw, V_SILL, f)
    # -- The loggia: plain piers at its corners, inlaid piers either side of
    # the middle bay, the beam over them, the side walls with the side
    # door's opening, and the back wall.
    y0, y1 = V_PIERS
    for x in (-(hw - 0.5), hw - 0.5):
        inlaid_pier(b, x, y0, y1, hall, hall + V_PIER, stripes=False)
    for x in (-2.4, 2.4):
        inlaid_pier(b, x, y0, y1, hall, hall + V_PIER)
    top = hall + V_CEIL + 0.4
    box(b, (-hw, y0, hall + V_PIER), (hw, y1, top), "marble", skip="+z", name="Beam")
    for s in (-1, 1):
        x0, x1 = sorted((s * (hw - 0.5), s * hw))
        box(b, (x0, y1, hall), (x1, V_COURT, top), "marble", skip="-z +z -y", name="Wall")
        b.collide("wall", (x0, y1, hall), (x1, V_COURT, top))
    box(b, (-hw + 0.5, V_BACK, hall), (hw - 0.5, V_COURT, top), "marble", skip="-z +z -x +x", name="Wall")
    b.collide("back", (-hw, V_BACK, hall), (hw, V_COURT, top))
    # The roof: a flat slab with a thin fascia, over the loggia.
    box(b, (-hw - 0.35, y0 - 0.35, top), (hw + 0.35, V_COURT + 0.35, top + 0.35), "lime", name="Roof")
    if not FAR:
        box(b, (-hw - 0.4, y0 - 0.4, top + 0.35), (hw + 0.4, V_COURT + 0.4, top + 0.45), "shade", skip="-z",
            name="Fascia")
    b.roofs.append(((0.0, (y0 + V_COURT) / 2), True, ((V_COURT - y0) / 2 + 0.4, hw + 0.4), top + 0.45, top + 0.46))
    # -- The entry court: the colonnade on its far edge under a beam and a
    # roof, the portal wall with the mahogany door in its stepped inlay,
    # the meander in the floor, and terracotta pots.
    for x in (-6.8, -2.4, 2.4, 6.8):
        column(b, x, V_COLS, hall, hall + 5.4, d=0.62)
    box(b, (-7.4, V_COLS - 0.45, hall + 5.4), (7.4, V_COLS + 0.45, hall + 6.0), "lime", skip="+z", name="Beam")
    box(b, (-8.0, V_COURT, hall + 6.0), (8.0, V_COLS + 0.6, hall + 6.4), "lime", name="CourtRoof")
    b.roofs.append(((0.0, (V_COURT + V_COLS) / 2), True, ((V_COLS - V_COURT) / 2 + 0.6, 8.0), hall + 6.4,
                    hall + 6.41))
    portal_face = local((0, V_COURT, 0), 180.0)
    dw, dh = 2.6, 3.6
    for lo, hi in (((-dw / 2 - 0.15, V_COURT, hall), (-dw / 2, V_COURT + 0.14, hall + dh)),
                   ((dw / 2, V_COURT, hall), (dw / 2 + 0.15, V_COURT + 0.14, hall + dh)),
                   ((-dw / 2 - 0.15, V_COURT, hall + dh), (dw / 2 + 0.15, V_COURT + 0.14, hall + dh + 0.15))):
        box(b, lo, hi, "bronze", skip="-y -z", name="DoorFrame")
    for s in (-1, 1):
        leaf = portal_face @ Matrix.Translation(Vector((s * dw / 4, 0.02, hall)))
        mahogany_leaf(b, leaf @ Matrix.Translation(Vector((0.0, -0.1, 0.0))), dw / 2 - 0.02, dh, pull_left=s > 0)
    stepped_surround(b, portal_face @ Matrix.Translation(Vector((0.0, 0.0, hall))), dw, dh)
    if not FAR:
        flat = Matrix.Translation(Vector((0.0, 0.0, hall + 0.01))) @ FLAT
        slab(b, -hw, hw, V_COURT, V_END, hall + 0.01, "marble", name="CourtFloor")
        meander_floor(b, flat, 0.0, V_COURT, r=3.0, out=1.0)
    for s in (-1, 1):
        terracotta_pot(b, s * 3.4, V_COURT + 0.5, hall, r=0.32, shrub=1.0, tree=True)
        terracotta_pot(b, s * 4.1, V_COURT + 0.45, hall, r=0.24, shrub=0.6)
        terracotta_pot(b, s * 9.3, V_COLS - 1.2, hall + 0.95, r=0.18, shrub=0.5)
        b.collide("pot", (s * 3.4 - 0.45, V_COURT, hall), (s * 3.4 + 0.45, V_COURT + 0.9, hall + 1.6))
        # Uplights washing the portal.
        uplight(b, s * 2.7, V_COURT + 0.6, hall, toward=(0.0, -1.0))
    # Two steps down at the court's back, to the trail.
    if not FAR:
        for i, (ya, z) in enumerate(((V_END, hall - 0.17), (V_END + 0.4, hall - 0.34))):
            box(b, (-tw, ya, -3.0), (tw, ya + 0.4, z), "marble", skip="-z -y", name="Step")
    b.front = (0.0, -1.0)


def belvedere_loggia(b):
    """The loggia's inside: a pale floor with dark border lines, the
    lintel band over the piers, a coffered marble ceiling with copper lines,
    louvers high on the side walls, the copper relief over a cushioned
    bench, urn trees, the side door, and sconces."""
    hall = V_HALL
    y0, y1 = V_PIERS[1], V_BACK
    x0, x1 = -(V_HW - 0.5), V_HW - 0.5
    b.inside = (0.0, (y0 + y1) / 2)
    if FAR:
        return
    slab(b, x0, x1, V_PIERS[0], y1, hall + 0.01, "marble", name="Floor")
    flat = Matrix.Translation(Vector((0.0, 0.0, hall + 0.01))) @ FLAT
    kw = {"mat": "walnut", "width": 0.05, "proud": 0.004}
    for inset in (0.35, 0.55):
        polyline(b, flat, [(0.0, y0 + inset), (x1 - inset, y0 + inset), (x1 - inset, y1 - inset), (0.0, y1 - inset)],
                 **kw)
    # The lintel band, on the beam's inner face over the piers.
    inner = local((0, y0, 0), 180.0)
    lintel_band(b, inner, 2 * x1 - 0.3, hall + V_PIER + 0.15, h=1.1)
    # The ceiling: a marble cove round a flat field with copper lines.
    ceil = hall + V_CEIL
    slab(b, x0, x1, V_PIERS[0], y1, ceil, "marble", down=True, name="Ceiling")
    for lo, hi in (((x0, y0, ceil - 0.28), (x1, y0 + 0.5, ceil)), ((x0, y1 - 0.5, ceil - 0.28), (x1, y1, ceil)),
                   ((x0, y0, ceil - 0.28), (x0 + 0.5, y1, ceil)), ((x1 - 0.5, y0, ceil - 0.28), (x1, y1, ceil))):
        box(b, lo, hi, "marble", skip="+z", name="Cove")
    down = Matrix.Translation(Vector((0.0, 0.0, ceil - 0.002))) @ Matrix.Rotation(math.radians(90.0), 4, "X")
    kc = {"mat": "copper", "width": 0.035, "proud": 0.004}
    cy = (y0 + y1) / 2
    polyline(b, down, [(0.0, -(y0 + 0.8)), (x1 - 0.8, -(y0 + 0.8)), (x1 - 0.8, -(y1 - 0.8)), (0.0, -(y1 - 0.8))], **kc)
    r = (y1 - y0) / 2 - 1.4
    for s in (-1, 1):
        arc(b, down, (s * (x1 - 1.4 - r), -cy), r, -math.pi / 2, math.pi / 2, 8, **kc) if s > 0 else \
            arc(b, down, (s * (x1 - 1.4 - r), -cy), r, math.pi / 2, 3 * math.pi / 2, 8, **kc)
    polyline(b, down, [(x1 - 1.4 - r, -(cy - r)), (0.0, -(cy - r))], **kc)
    polyline(b, down, [(x1 - 1.4 - r, -(cy + r)), (0.0, -(cy + r))], **kc)
    # Louvers high on both side walls.
    for s in (-1, 1):
        side = local((s * x1, 0, 0), -90 * s)
        u0, u1 = sorted((-18.6 * s, -24.6 * s))
        louver(b, side, u0, u1, hall + 3.9, hall + 5.6)
    # The relief over the bench on the +x wall, left of the view, and
    # the side door on the -x wall, right of it.
    east = local((x1, 0, 0), -90.0)
    relief_panel(b, east, -21.4, hall + 1.3)
    cushioned_bench(b, x1 - 0.7, x1, 19.4, 23.4, hall, back="+x")
    west = local((x0, 0, 0), 90.0)
    side_door(b, west, 22.6, hall)
    for s in (-1, 1):
        urn_tree(b, s * 5.9, y0 + 0.9, hall, h=2.6)
        sconce(b, local((0, y1, 0), 0.0), s * 2.4, hall + 2.4)


def new(name):
    b = bl.Building(name, {"plaster": "cream", "roof": "red", "timber": "dark"})
    b.chimneys = []
    b.lights = []
    b.flames = []
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


@model
def civic_hall():
    b = new("civic_hall")
    civic_body(b)
    civic_chamber(b)
    return b


@model
def belvedere():
    b = new("belvedere")
    belvedere_body(b)
    belvedere_loggia(b)
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
def workstation_piece():
    b = new("workstation")
    workstation(b, 0, 0, 0)
    return b


@piece
def reception_desk_piece():
    b = new("reception_desk")
    reception_desk(b, 0, 0, 0)
    return b


@piece
def reception_chair_piece():
    b = new("reception_chair")
    reception_chair(b, 0, 0, 0)
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


# The Civic Hall's pieces.


@piece
def bronze_column_piece():
    b = new("bronze_column")
    bronze_column(b, 0, 0, 0, 8.4)
    return b


@piece
def dentil_cornice_bay():
    b = new("dentil_cornice_bay")
    dentil_cornice(b, -2.0, 2.0, 0.0, 1.2, 0.0, sides=("-y",))
    return b


@piece
def attic_piece():
    b = new("attic")
    attic(b, -2.0, 2.0, 0.0, 2.0, 0.0)
    return b


@piece
def circuit_portal():
    b = new("circuit_portal")
    box(b, (-3.4, 0.0, 0.0), (-2.5, 1.6, 8.0), "lime", skip="-z", name="Wall")
    box(b, (2.5, 0.0, 0.0), (3.4, 1.6, 8.0), "lime", skip="-z", name="Wall")
    box(b, (-2.5, 0.0, 7.2), (2.5, 1.6, 8.0), "lime", name="Wall")
    portal(b, 0.0, 0.0, 0.0)
    return b


@piece
def paired_window():
    b = new("paired_window")
    paired_windows(b, local((0, 0, 0)), 0.0, 0.0, h=3.1)
    return b


@piece
def bowl_planter():
    b = new("bowl_planter")
    bowl_wall(b, -1.2, 1.2, 0.0, 2.4, 0.0, 0.75)
    bowl(b, 0.0, 1.2, 0.75, r=0.75)
    return b


@piece
def council_ring_piece():
    b = new("council_ring")
    council_ring(b, (0.0, 0.0), 0.0)
    return b


# The belvedere's pieces.


@piece
def inlaid_pier_piece():
    b = new("inlaid_pier")
    inlaid_pier(b, 0.0, 0.0, 1.0, 0.0, 4.4)
    return b


@piece
def lintel_band_piece():
    b = new("lintel_band")
    lintel_band(b, local((0, 0, 0)), 10.0, 0.0)
    return b


@piece
def louver_piece():
    b = new("louver")
    louver(b, local((0, 0, 0)), -1.5, 1.5, 0.0, 1.7)
    return b


@piece
def relief_panel_piece():
    b = new("relief_panel")
    relief_panel(b, local((0, 0, 0)), 0.0, 0.0)
    return b


@piece
def cushioned_bench_piece():
    b = new("cushioned_bench")
    cushioned_bench(b, -1.6, 1.6, 0.0, 0.7, 0.0, back="+y")
    return b


@piece
def urn_tree_piece():
    b = new("urn_tree")
    urn_tree(b, 0.0, 0.0, 0.0)
    return b


@piece
def threshold_piece():
    b = new("threshold")
    threshold(b, -2.0, 2.0, 0.0, 0.0)
    return b


@piece
def mahogany_door():
    b = new("mahogany_door")
    for s in (-1, 1):
        mahogany_leaf(b, local((s * 0.65, 0, 0)), 1.28, 3.6, pull_left=s > 0)
    return b


@piece
def stepped_surround_piece():
    b = new("stepped_surround")
    box(b, (-3.6, 0.0, 0.0), (3.6, 0.2, 6.0), "lime", skip="-z +y", name="Wall")
    stepped_surround(b, local((0, 0, 0)), 2.6, 3.6)
    return b


@piece
def meander_floor_piece():
    b = new("meander_floor")
    slab(b, -3.4, 3.4, -3.4, 0.0, 0.0, "marble", name="Floor")
    meander_floor(b, FLAT, 0.0, 0.0, r=3.0)
    return b


@piece
def terracotta_pot_piece():
    b = new("terracotta_pot")
    terracotta_pot(b, -0.5, 0.0, 0.0, r=0.32, shrub=1.0, tree=True)
    terracotta_pot(b, 0.5, 0.0, 0.0, r=0.24, shrub=0.6)
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
    if b.lights or b.flames:
        # Blender (x, y, z) is glTF (x, z, -y).
        path = os.path.join(out, b.name + ".footprint.json")
        data = json.load(open(path))
        data["lights"] = [{"kind": k, "at": [x, z, round(-y, 3) + 0.0]} for k, (x, y, z) in b.lights]
        data["flames"] = [[x, z, round(-y, 3) + 0.0] for x, y, z in b.flames]
        with open(path, "w") as f:
            json.dump(data, f, indent=2)
            f.write("\n")
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
