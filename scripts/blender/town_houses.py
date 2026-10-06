"""Build Everglade's lighter town houses in the Medieval Village MegaKit's style.

Run headless:
    Blender -b --factory-startup --python scripts/blender/town_houses.py -- \
        [OUT_DIR] [NAME ...] [--kit KIT_DIR]

Reference mode: every house is built from boxes, slabs, and triangles, not
from the kit's wall pieces, and samples only the kit's base-color images
(plaster, round tiles, timber, uneven brick, rock trim), recolored the way
`buildings.py` recolors them. A kit-built house of the same size costs 8,000
to 13,000 triangles; these cost 1,500 to 3,500, so the town can hold more
of them and more kinds.

Style rules, read from the kit's pieces and `buildings.py`'s houses:

- Storeys about 3 m apart, stone plinths or stone ground floors under
  plaster, and dark timber framing in a regular rhythm: corner posts,
  rails at the sills and heads, posts between the windows, and braces in
  the end panels.
- Steep roofs with deep overhangs: gables, hips, and gambrels, in the
  kit's round-tile image, with a dark ridge.
- Upper floors that jut over the street, window frames proud of the wall,
  sills, shutters, flower boxes, and chimneys of stone.

Every house's plaster and roof tiles are named `HousePlaster` and
`HouseTiles`, so the zone paints each placed house in its own colors
(`layout::paint`), as it paints the kit-built houses; the colors here are
a house's own when the zone leaves it unpainted.

Each model is a whole house, written to `OUT_DIR/<name>.glb` with a
`<name>.footprint.json` beside it, the format `buildings.py` writes. Frame:
1 unit = 1 m; fronts face -Y in Blender, which is +Z after export; the
origin is on the ground at the center of the front wall. OUT_DIR defaults
to `assets/verse/generated/buildings`.

- `shop_house`: a narrow shop under a front gable, with two display
  windows whose alcoves show goods on shelves through clear glass, an
  awning, a painted fascia, and a jettied, half-timbered upper floor.
- `gambrel_house`: a gambrel roof with its gable to the street, a porch,
  and windows in the gambrel's gable.
- `stone_cottage`: a stone cottage under a hipped roof with a dormer and
  an outside chimney stack.
- `brownstone`: three storeys of brown stone over a raised basement, a
  stoop with iron rails, a door hood, a bracketed cornice, and a flat
  roof.
- `timber_house`: a stone ground floor under a jettied timber-framed
  upper floor, a balcony, and a cross gable over the front.
- `lantern_inn`: the Lantern Quarter's inn, with warm lamplit windows,
  wall lanterns, a hanging sign, and a hipped roof with dormers.

Each house also gets a far level of detail, `OUT_DIR/far/<name>.glb`: the
same walls, roofs, timbers, and chimneys with each window and door one
pane, and without the goods, signs, lanterns, joists, rails, and braces,
at a fifth to a quarter of the triangles. `everglade_admit.py` admits it
as the pack's `lod/generated.<name>`.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(__file__))
import buildings as bl  # noqa: E402

DARK = bl.DARK_WOOD
LIGHT = bl.LIGHT_WOOD
# Whether the far level of detail is being built: the details a viewer
# can't see beyond 80 m are left out (`zones::everglade::detail`).
FAR = False


def extra_materials(b, shop=(0.20, 0.36, 0.26), shutter=(0.16, 0.30, 0.22), stone_tint=None):
    """The materials these houses add to `buildings.py`'s."""
    m = b.mats
    m["shop"] = bl._material("ShopPaint", color=shop, rough=0.6)
    m["shutter"] = bl._material("ShutterPaint", color=shutter, rough=0.7)
    # The pack holds at most 16 materials a model, so the rest reuse
    # `buildings.py`'s colors: lamplit windows its lamp glass, the fascia
    # iron, gilt and goods its sign paint and cloths, flowers cloth and
    # lamp glass.
    m["lit"] = m["lamp"]
    m["fascia"] = m["iron"]
    m["gilt"] = m["paint"]
    m["flower_red"] = m["cloth"]
    m["flower_pink"] = m["lamp"]
    m["goods_a"] = m["paint"]
    m["goods_b"] = m["cloth"]
    m["goods_c"] = m["cloth2"]
    # The materials the zone repaints per house (`scene::PAINTED`).
    m["plaster"].name = "HousePlaster"
    m["tiles"].name = "HouseTiles"
    if stone_tint is not None:
        px = bl._pixels(os.path.join(bl.KIT, "glTF", "T_UnevenBrick_BaseColor.png"), 512)
        img = bl._image("T_UnevenBrick_tinted", bl._recolor(px, stone_tint))
        m["stone"] = bl._material("StoneTinted", img)


class Face:
    """One wall face: local x along it (left to right seen from outside),
    local z up, local -y out of the wall."""

    def __init__(self, b, rot, origin):
        self.b = b
        self.m = bl.frame(rot, origin)

    def box(self, lo, hi, mat, name="Part", band=None, tile=2.0):
        """A box on the face, without its side toward the wall (local +y),
        which the wall hides: a sixth less geometry for every frame, sill,
        shutter, and timber."""
        mesh = bl.box_mesh(name, lo, hi, self.b.mats[mat], band=band, tile=tile)
        bm = bmesh.new()
        bm.from_mesh(mesh)
        bm.normal_update()
        back = [f for f in bm.faces if f.normal.y > 0.9]
        bmesh.ops.delete(bm, geom=back, context="FACES")
        bmesh.ops.transform(bm, matrix=self.m, verts=bm.verts)
        bm.to_mesh(mesh)
        bm.free()
        return self.b.solid(name, mesh)

    def beam(self, lo, hi, band=DARK, name="Timber"):
        return self.box(lo, hi, "wood", name=name, band=band)

    def brace(self, u0, z0, u1, z1, w=0.16, out=0.05):
        """A diagonal timber from (u0, z0) to (u1, z1) on the face."""
        du, dz = u1 - u0, z1 - z0
        length = math.hypot(du, dz)
        xf = self.m @ Matrix.Translation(Vector((u0, 0, z0))) @ Matrix.Rotation(-math.atan2(dz, du), 4, "Y")
        mesh = bl.box_mesh("Brace", (0, -out, -w / 2), (length, 0.0, w / 2), self.b.mats["wood"], band=DARK, xf=xf)
        self.b.solid("Brace", mesh)

    def window(self, u, z, w, h, glass="glass", shutters=True, flowers=False, cross=True, frame="wood"):
        """A window whose frame stands proud of the wall, with its sill."""
        t = 0.09
        if FAR:
            self.box((u - w / 2 - t, -0.04, z - t), (u + w / 2 + t, 0.0, z + h + t), glass, name="Glass")
            return
        self.box((u - w / 2, -0.03, z), (u + w / 2, 0.0, z + h), glass, name="Glass")
        band = DARK if frame == "wood" else None
        for lo, hi in (((u - w / 2 - t, -0.09, z + h), (u + w / 2 + t, 0.0, z + h + t)),
                       ((u - w / 2 - t, -0.09, z - t), (u + w / 2 + t, 0.0, z)),
                       ((u - w / 2 - t, -0.09, z), (u - w / 2, 0.0, z + h)),
                       ((u + w / 2, -0.09, z), (u + w / 2 + t, 0.0, z + h))):
            self.box(lo, hi, frame, name="Frame", band=band)
        if cross:
            self.box((u - 0.03, -0.06, z), (u + 0.03, 0.0, z + h), frame, name="Mullion", band=band)
            self.box((u - w / 2, -0.06, z + h * 0.62 - 0.03), (u + w / 2, 0.0, z + h * 0.62 + 0.03), frame,
                     name="Mullion", band=band)
        self.box((u - w / 2 - 0.16, -0.2, z - t - 0.08), (u + w / 2 + 0.16, 0.0, z - t), "wood", name="Sill",
                 band=LIGHT)
        if shutters:
            for s in (-1, 1):
                x0 = u + s * (w / 2 + t)
                x1 = x0 + s * w / 2
                self.box((min(x0, x1), -0.08, z), (max(x0, x1), -0.03, z + h), "shutter", name="Shutter")
        if flowers:
            self.box((u - w / 2, -0.42, z - t - 0.4), (u + w / 2, -0.2, z - t - 0.08), "wood", name="FlowerBox",
                     band=DARK)
            self.box((u - w / 2 + 0.04, -0.4, z - t - 0.08), (u + w / 2 - 0.04, -0.22, z - t + 0.1), "leaf",
                     name="Leaves")
            for k in range(3):
                x = u - w / 2 + w * (k + 0.5) / 3
                mat = "flower_red" if k % 2 == 0 else "flower_pink"
                self.box((x - 0.09, -0.36, z - t + 0.04), (x + 0.09, -0.24, z - t + 0.2), mat, name="Bloom")

    def door(self, u, w, h, z=0.0, glazed=False, leaf="wood"):
        """A plank door set into the wall under a frame and lintel."""
        if FAR:
            self.box((u - w / 2 - 0.12, -0.04, z), (u + w / 2 + 0.12, 0.1, z + h + 0.16), leaf, name="Door",
                     band=DARK if leaf == "wood" else None)
            return
        self.box((u - w / 2, -0.04, z), (u + w / 2, 0.1, z + h), leaf, name="Door",
                 band=DARK if leaf == "wood" else None)
        if glazed:
            self.box((u - w / 2 + 0.15, -0.06, z + h * 0.55), (u + w / 2 - 0.15, -0.03, z + h - 0.15), "glass",
                     name="DoorGlass")
        t = 0.12
        for lo, hi in (((u - w / 2 - t, -0.1, z), (u - w / 2, 0.0, z + h)),
                       ((u + w / 2, -0.1, z), (u + w / 2 + t, 0.0, z + h)),
                       ((u - w / 2 - t - 0.06, -0.14, z + h), (u + w / 2 + t + 0.06, 0.0, z + h + 0.16))):
            self.box(lo, hi, "wood", name="DoorFrame", band=DARK)
        self.box((u + w / 2 - 0.2, -0.06, z + h * 0.48), (u + w / 2 - 0.12, 0.0, z + h * 0.52), "iron", name="Knob")

    def framing(self, z0, z1, u0, u1, posts, braces=(), rails=()):
        """Half-timbering on a plaster storey: posts at `posts`, rails at the
        heights in `rails`, the head and sill plates, and braces."""
        for u in posts:
            self.beam((u - 0.09, -0.05, z0), (u + 0.09, 0.0, z1))
        for z in (z0, z1 - 0.16) + tuple(rails):
            self.beam((u0, -0.05, z), (u1, 0.0, z + 0.16))
        for b in () if FAR else braces:
            self.brace(*b)


def chimney(b, x, y, z0, top, w=0.7):
    """A stone stack from inside the roof to `top`, with a cap. Returns its
    top in the glTF frame, for the zone's smoke."""
    b.block((x - w / 2, y - w / 2, z0), (x + w / 2, y + w / 2, top), "stone", name="Chimney", scale=1.2)
    b.block((x - w / 2 - 0.08, y - w / 2 - 0.08, top), (x + w / 2 + 0.08, y + w / 2 + 0.08, top + 0.14), "stone",
            name="ChimneyCap", scale=1.0)
    b.block((x - 0.18, y - 0.18, top + 0.14), (x + 0.18, y + 0.18, top + 0.4), "iron", name="Pot", scale=1.0)
    b.chimneys.append((round(x, 2), round(top + 0.4, 2), round(-y, 2)))


def slab_walls(b, w, d, z0, z1, mat="plaster", front=True, y0=0.0, thick=0.3):
    """Four plain walls of a storey, the front at y0 and the back at d."""
    if front:
        b.block((-w / 2, y0, z0), (w / 2, y0 + thick, z1), mat, name="Wall")
    b.block((-w / 2, d - thick, z0), (w / 2, d, z1), mat, name="Wall")
    for sx in (-1, 1):
        x0, x1 = sorted((sx * w / 2, sx * (w / 2 - thick)))
        b.block((x0, y0, z0), (x1, d, z1), mat, name="Wall")


def corner_posts(b, w, d, z0, z1, y0=0.0):
    for x in (-w / 2, w / 2):
        for y in (y0, d):
            b.beam((x - 0.11, y - 0.11, z0), (x + 0.11, y + 0.11, z1), band=DARK, name="Post")


def jetty_joists(b, w, z, out, y=0.0):
    """Joist ends under a floor that juts `out` past the wall below."""
    n = max(3, int(round(w / 0.9)))
    for i in range(0 if FAR else n + 1):
        x = -w / 2 + 0.15 + (w - 0.3) * i / n
        b.beam((x - 0.08, y - out, z - 0.18), (x + 0.08, y + 0.05, z), band=DARK, name="Joist")
    b.beam((-w / 2 - 0.05, y - out - 0.05, z - 0.05), (w / 2 + 0.05, y - out + 0.15, z + 0.1), band=DARK,
           name="Bressumer")


def dormer(b, x, y, z, w=1.5, h=1.4, depth=1.8, glass="glass"):
    """A gabled dormer whose front stands at y on a slope that meets height
    z there, its window toward -Y."""
    b.block((x - w / 2, y, z - 0.3), (x + w / 2, y + depth, z + h), "plaster", name="Dormer")
    face = Face(b, 0, (0, y, 0))
    face.window(x, z + 0.25, w - 0.6, h - 0.5, glass=glass, shutters=False, cross=True)
    bl.slab_roof(b, w + 0.1, depth, z + h, 0.75, center=(x, y + depth / 2), along_x=False, over=0.18,
                 thick=0.12)


def save(b, out):
    # One object, so the glTF has one node: the pack bounds a model's nodes.
    objs = [o for o in b.col.objects if o.type == "MESH"]
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    bpy.context.object.name = b.name
    used = {s.material.name for s in bpy.context.object.material_slots if s.material}
    print(f"MATERIALS {b.name} {len(used)}")
    assert len(used) <= 16, sorted(used)
    b.save(out)
    for top in b.chimneys:
        print(f"CHIMNEY {b.name} {top[0]} {top[1]} {top[2]}")


def save_far(b, out):
    """The far level: one object, glb only."""
    objs = [o for o in b.col.objects if o.type == "MESH"]
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    bpy.context.object.name = b.name
    os.makedirs(out, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=os.path.join(out, b.name + ".glb"), export_format="GLB",
                              export_yup=True, export_apply=True, export_image_format="JPEG",
                              export_jpeg_quality=88, export_tangents=False, export_cameras=False,
                              export_lights=False, export_extras=False)
    print(f"FAR {b.name} triangles={b.triangles()}")


def house(name, scheme, **extra):
    b = bl.Building(name, scheme)
    b.chimneys = []
    extra_materials(b, **extra)
    return b


HOUSES = {}


def model(fn):
    HOUSES[fn.__name__] = fn
    return fn


# --------------------------------------------------------------------------
# A shop under a front gable


def shop_house_body(name, scheme, shop, shutter, awning_cloth):
    b = house(name, scheme, shop=shop, shutter=shutter)
    w, d, g, up, jet = 7.6, 9.0, 3.2, 2.9, 0.45
    hw = w / 2
    front = Face(b, 0, (0, 0, 0))
    # The shop front: painted piers, risers, and fascia round two display
    # windows and the door, each window an alcove of goods behind glass.
    for u0, u1 in ((-hw, -3.1), (-0.9, -0.6), (0.6, 0.9), (3.1, hw)):
        front.box((u0, 0.0, 0.0), (u1, 0.3, g), "shop", name="Pier")
    for u0, u1 in ((-3.1, -0.9), (0.9, 3.1)):
        front.box((u0, 0.0, 0.0), (u1, 0.3, 0.65), "shop", name="Riser")
        if FAR:
            front.box((u0, 0.0, 0.65), (u1, 0.08, 2.55), "glass", name="ShopGlass")
            continue
        front.box((u0, 0.3, 0.0), (u1, 1.0, 0.65), "wood", name="Stage", band=LIGHT)
        front.box((u0, 0.3, 2.55), (u1, 1.0, 2.62), "wood", name="AlcoveTop", band=DARK)
        front.box((u0, 0.95, 0.65), (u1, 1.05, 2.55), "wood", name="AlcoveBack", band=DARK)
        for uu in (u0, u1):
            front.box((uu - 0.05, 0.3, 0.65), (uu + 0.05, 1.0, 2.55), "shop", name="AlcoveSide")
        for z in (1.25, 1.85):
            front.box((u0 + 0.05, 0.55, z - 0.04), (u1 - 0.05, 0.95, z), "wood", name="Shelf", band=LIGHT)
        goods = ("goods_a", "goods_b", "goods_c", "cloth4", "goods_a", "cloth3")
        for k, z in enumerate((0.65, 1.25, 1.85)):
            for j in range(3):
                uu = u0 + (u1 - u0) * (j + 0.5) / 3
                hh = 0.22 + 0.1 * ((j + k) % 2)
                front.box((uu - 0.2, 0.62, z), (uu + 0.2, 0.9, z + hh), goods[(j + 2 * k) % 6], name="Goods")
        front.box((u0, 0.06, 0.65), (u1, 0.08, 2.55), "glass_clear", name="ShopGlass")
        front.box(((u0 + u1) / 2 - 0.035, -0.02, 0.65), ((u0 + u1) / 2 + 0.035, 0.06, 2.55), "shop",
                  name="GlazingBar")
        front.box((u0, -0.02, 2.0), (u1, 0.06, 2.06), "shop", name="GlazingBar")
        front.box((u0 - 0.05, -0.12, 0.6), (u1 + 0.05, 0.02, 0.68), "wood", name="Sill", band=LIGHT)
    front.box((-0.6, 0.0, 2.4), (0.6, 0.3, g), "shop", name="OverDoor")
    front.box((-hw, 0.0, 2.55), (hw, 0.3, g), "shop", name="FasciaWall")
    front.box((-3.0, -0.12, 2.68), (3.0, 0.0, 3.08), "fascia", name="Fascia")
    front.box((-2.2, -0.15, 2.82), (2.2, -0.11, 2.94), "gilt", name="Lettering")
    front.door(0.0, 1.1, 2.3, glazed=True)
    # Side and back walls of the shop floor, and the upper floor.
    slab_walls(b, w, d, 0.0, g, front=False)
    b.block((-hw - 0.05, d - 0.35, 0.0), (hw + 0.05, d + 0.05, 0.5), "stone", name="Plinth", scale=1.5)
    jetty_joists(b, w, g, jet)
    slab_walls(b, w, d, g, g + up, y0=-jet)
    top = g + up
    upper = Face(b, 0, (0, -jet, 0))
    for u in (-2.2, 0.0, 2.2):
        wide = 0.7 if u == 0.0 else 1.05
        upper.window(u, g + 0.9, wide, 1.3, shutters=u != 0.0, flowers=u != 0.0)
    upper.framing(g, top, -hw, hw, (-hw + 0.1, -1.2, 1.2, hw - 0.1), rails=(g + 0.62,),
                  braces=((-hw + 0.2, g + 0.16, -3.1, g + 0.6), (hw - 0.2, g + 0.16, 3.1, g + 0.6)))
    for side, rot in ((-1, -90), (1, 90)):
        f = Face(b, rot, (side * hw, d / 2, 0))
        f.window(-side * 1.6, g + 0.9, 0.9, 1.2, shutters=False)
        f.window(-side * 1.6, 0.9, 0.9, 1.3, shutters=False)
        f.framing(g, top, -(d + jet) / 2, (d - jet) / 2 - 0.0, (0.0,), rails=(g + 0.62,))
    back = Face(b, 180, (0, d, 0))
    for u in (-1.8, 1.8):
        back.window(u, 0.9, 1.0, 1.3)
        back.window(u, g + 0.9, 1.0, 1.2)
    back.door(0.0, 1.0, 2.2)
    corner_posts(b, w, d, g, top, y0=-jet)
    bl.awning(b, -3.15, 3.15, -0.02, 2.62, depth=1.2, drop=0.45)
    for o in b.col.objects:
        if o.name.startswith("Awning.") and o.data.materials and o.data.materials[0] == b.mats["cloth"]:
            o.data.materials[0] = b.mats[awning_cloth]
    # A steep gable to the street, its face framed in timber.
    span, length = w + 0.1, d + jet
    ridge = bl.slab_roof(b, span, length, top + 0.02, 3.7, center=(0, (d - jet) / 2), over=0.55, thick=0.2)
    gable = Face(b, 0, (0, -jet - 0.01, 0))
    gable.beam((-0.09, -0.05, top), (0.09, 0.0, ridge - 0.3))
    gable.beam((-2.0, -0.05, top + 1.4), (2.0, 0.0, top + 1.56))
    gable.window(0.0, top + 0.35, 0.6, 0.85, shutters=False, cross=False)
    gable.brace(-3.6, top + 0.1, -0.1, ridge - 0.4)
    gable.brace(3.6, top + 0.1, 0.1, ridge - 0.4)
    chimney(b, 1.9, d - 1.6, top, ridge + 0.6)
    if not FAR:
        bl.hanging_sign(b, -hw + 0.3, -jet, g - 0.1, rot=0, reach=1.1)
    b.collide("house", (-hw - 0.1, -0.1, 0), (hw + 0.1, d + 0.1, top))
    b.roofs.append(((0.0, (d - jet) / 2), False, (span / 2 + 0.55, length / 2 + 0.55), top, ridge))
    b.front = (0.0, -1.0)
    return b


@model
def shop_house():
    """A narrow shop in rose plaster under red tiles, its front painted
    green, under a green-striped awning."""
    return shop_house_body("shop_house", {"plaster": "rose", "roof": "red", "timber": "dark"},
                           shop=(0.16, 0.32, 0.22), shutter=(0.16, 0.30, 0.22), awning_cloth="cloth3")


# --------------------------------------------------------------------------
# A gambrel house


@model
def gambrel_house():
    """A house under a gambrel roof with its gable to the street: a porch
    over the door, and a pair of windows in the gable."""
    b = house("gambrel_house", {"plaster": "sage", "roof": "charcoal", "timber": "dark"},
              shutter=(0.86, 0.84, 0.76))
    w, d, wall = 8.0, 8.0, 3.4
    hw = w / 2
    b.block((-hw - 0.12, -0.12, 0.0), (hw + 0.12, d + 0.12, 0.6), "rock", name="Plinth", scale=1.5)
    slab_walls(b, w, d, 0.6, wall)
    front = Face(b, 0, (0, 0, 0))
    front.door(0.0, 1.1, 2.25, z=0.6)
    for u in (-2.3, 2.3):
        front.window(u, 1.5, 1.1, 1.4, flowers=True)
    for side, rot in ((-1, -90), (1, 90)):
        f = Face(b, rot, (side * hw, d / 2, 0))
        for u in (-1.8, 1.8):
            f.window(u, 1.5, 1.0, 1.3)
    back = Face(b, 180, (0, d, 0))
    for u in (-2.0, 2.0):
        back.window(u, 1.5, 1.0, 1.3)
    corner_posts(b, w, d, 0.6, wall)
    for x in (-hw, hw):
        for y in (0.0, d):
            b.block((x - 0.13, y - 0.13, 0.6), (x + 0.13, y + 0.13, wall), "white_paint", name="CornerBoard")
    # The gambrel: steep below the knees, shallow above.
    eave, knee, ridge = (hw, wall), (hw - 1.5, wall + 2.4), wall + 3.6
    for side in (-1, 1):
        lower = ((side * eave[0], eave[1]), (side * knee[0], knee[1]))
        upper = ((side * knee[0], knee[1]), (0.0, ridge))
        for (a, c), over in ((lower, 0.5), (upper, 0.0)):
            if side > 0:
                a, c = c, a
            before, after = (over, 0.0) if side < 0 else (0.0, over)
            bl.gambrel_slab(b, a, c, d + 0.9, d / 2, "tiles", before=before, after=after)
    b.beam((-0.15, -0.5, ridge - 0.1), (0.15, d + 0.5, ridge + 0.12), band=DARK, name="Ridge")
    for y, flip in ((0.0, False), (d, True)):
        pent = [(-hw, y, wall), (hw, y, wall), (knee[0], y, knee[1] - 0.1), (0.0, y, ridge - 0.12),
                (-knee[0], y, knee[1] - 0.1)]
        b.solid("Gable", bl.poly_mesh("Gable", [pent[::-1] if flip else pent], b.mats["plaster"]))
    for u in (-1.1, 1.1):
        front.window(u, wall + 0.6, 0.8, 1.2, shutters=True)
        back.window(u, wall + 0.6, 0.8, 1.1, shutters=False)
    front.beam((-hw, -0.06, wall - 0.05), (hw, 0.0, wall + 0.12))
    front.window(0.0, wall + 2.3, 0.5, 0.55, shutters=False, cross=False)
    # The porch: two posts, a small gabled roof, and a step.
    for u in (-1.1, 1.1):
        front.beam((u - 0.09, -1.5, 0.6), (u + 0.09, -1.32, 2.9), band=LIGHT)
    front.box((-1.4, -1.6, 0.0), (1.4, 0.0, 0.6), "rock", name="PorchFloor", tile=1.5)
    front.box((-0.9, -2.0, 0.0), (0.9, -1.6, 0.3), "rock", name="Step", tile=1.5)
    bl.slab_roof(b, 2.8, 1.7, 2.9, 0.8, center=(0.0, -0.8), along_x=False, over=0.2, thick=0.12)
    chimney(b, 1.4, d - 1.5, wall, ridge + 0.7)
    b.collide("house", (-hw - 0.12, -0.12, 0), (hw + 0.12, d + 0.12, wall))
    b.collide("porch", (-1.4, -1.6, 0), (1.4, 0.0, 0.6))
    b.collide("loft", (-hw + 0.6, -0.12, 0), (hw - 0.6, d + 0.12, wall + 1.5))
    b.roofs.append(((0.0, d / 2), False, (knee[0], d / 2 + 0.45), knee[1], ridge))
    b.front = (0.0, -2.4)
    return b


# --------------------------------------------------------------------------
# A stone cottage under a hipped roof


@model
def stone_cottage():
    """A one-storey stone cottage under a steep hipped roof, with a dormer
    over the door and a stone chimney stack built up its side."""
    b = house("stone_cottage", {"plaster": "cream", "roof": "brown", "timber": "dark"},
              shutter=(0.22, 0.34, 0.52))
    w, d, wall = 8.0, 7.0, 3.1
    hw = w / 2
    slab_walls(b, w, d, 0.0, wall, mat="stone")
    b.block((-hw - 0.1, -0.1, 0.0), (hw + 0.1, d + 0.1, 0.35), "rock", name="Plinth", scale=1.5)
    front = Face(b, 0, (0, 0, 0))
    front.door(-0.4, 1.0, 2.2, glazed=False)
    front.box((-1.1, -0.12, 2.36), (0.3, 0.0, 2.6), "rock", name="Lintel", tile=1.0)
    for u in (-2.6, 1.9):
        front.window(u, 1.0, 1.1, 1.2, flowers=True)
        front.box((u - 0.75, -0.12, 2.3), (u + 0.75, 0.0, 2.52), "rock", name="Lintel", tile=1.0)
    back = Face(b, 180, (0, d, 0))
    for u in (-2.0, 2.0):
        back.window(u, 1.0, 1.0, 1.1)
    left = Face(b, -90, (-hw, d / 2, 0))
    left.window(0.0, 1.0, 1.0, 1.1)
    ridge = bl.hip_roof(b, w, d, wall + 0.05, 3.0, center=(0, d / 2), over=0.55)
    # The slope over the front wall rises 3 m over 3.5 m plus the overhang.
    run = d / 2 + 0.55
    zf = lambda y: wall + 0.05 + 3.0 * (y + 0.55) / run  # noqa: E731
    dormer(b, -0.4, 0.6, zf(0.6) - 0.05, w=1.6, h=1.3, depth=1.7)
    # The chimney stack climbs the east wall outside.
    b.block((hw, 2.4, 0.0), (hw + 0.7, 4.0, ridge - 0.6), "stone", name="Stack", scale=1.3)
    chimney(b, hw + 0.35, 3.2, ridge - 0.6, ridge + 0.7, w=0.8)
    b.collide("house", (-hw - 0.1, -0.1, 0), (hw + 0.1, d + 0.1, wall))
    b.collide("stack", (hw, 2.4, 0), (hw + 0.7, 4.0, ridge - 0.6))
    b.roofs.append(((0.0, d / 2), True, (d / 2 + 0.55, max(0.6, (w - d) / 2 + 0.6)), wall + 0.05, ridge))
    b.front = (-0.4, -0.9)
    return b


# --------------------------------------------------------------------------
# A brownstone with its stoop


@model
def brownstone():
    """Three storeys of brown stone over a raised basement, a stoop with
    iron rails up to a hooded door, tall windows, and a bracketed cornice
    over a flat roof."""
    b = house("brownstone", {"plaster": "terracotta", "roof": "charcoal", "timber": "dark"},
              shutter=(0.2, 0.18, 0.16), stone_tint=(0.50, 0.33, 0.25))
    w, d, base, storey = 8.0, 8.0, 1.4, 3.1
    hw = w / 2
    top = base + 3 * storey
    slab_walls(b, w, d, base, top, mat="stone")
    slab_walls(b, w, d, 0.0, base, mat="rock")
    front = Face(b, 0, (0, 0, 0))
    # The stoop: seven steps up to a landing before the door, with cheek
    # walls and iron rails.
    sx0, sx1, steps, rise, tread = 1.3, 3.1, 7, base / 7, 0.3
    for i in range(steps):
        y = -0.9 - tread * (steps - i)
        front.box((sx0, y, 0.0), (sx1, y + tread + 0.01, rise * (i + 1)), "rock", name="Step", tile=1.0)
    front.box((sx0, -0.9, 0.0), (sx1, 0.0, base), "rock", name="Landing", tile=1.0)
    foot = -0.9 - tread * steps
    for u in (sx0 - 0.2, sx1):
        front.box((u, foot, 0.0), (u + 0.2, -0.9, 0.45), "stone", name="Cheek")
        front.box((u, -0.9 - tread * 3, 0.0), (u + 0.2, -0.9, 0.95), "stone", name="Cheek")
        front.box((u, -0.9, 0.0), (u + 0.2, 0.0, base + 0.1), "stone", name="Cheek")
        uu = u + 0.1
        if FAR:
            continue
        for y, z in ((foot + 0.15, 0.45), (-0.9 - tread * 3, 0.95), (-0.95, base + 0.1)):
            front.box((uu - 0.03, y - 0.03, z), (uu + 0.03, y + 0.03, z + 0.95), "iron", name="Baluster")
        # The handrail climbs from the foot to the landing.
        y0, z0, y1, z1 = foot + 0.15, 1.35, -0.95, base + 1.0
        length = math.hypot(y1 - y0, z1 - z0)
        xf = (front.m @ Matrix.Translation(Vector((uu, y0, z0))) @
              Matrix.Rotation(math.atan2(z1 - z0, y1 - y0), 4, "X"))
        b.solid("Rail", bl.box_mesh("Rail", (-0.035, 0.0, -0.035), (0.035, length, 0.035), b.mats["iron"], xf=xf))
        front.box((uu - 0.035, -0.95, base + 0.97), (uu + 0.035, 0.0, base + 1.04), "iron", name="Rail")
    # The door under its hood, a transom over it.
    du = (sx0 + sx1) / 2
    front.door(du, 1.2, 2.5, z=base, glazed=False)
    front.box((du - 0.6, -0.02, base + 2.66), (du + 0.6, 0.02, base + 3.0), "glass", name="Transom")
    front.box((du - 0.95, -0.45, base + 3.05), (du + 0.95, 0.0, base + 3.25), "stone", name="Hood")
    for u in (du - 0.85, du + 0.85):
        front.box((u - 0.08, -0.38, base + 2.75), (u + 0.08, 0.0, base + 3.05), "stone", name="Corbel")
    # Tall windows with stone lintels and sills, the stoop's side short.
    for floor in range(3):
        z = base + floor * storey + 0.75
        for u in ((-2.4, -0.4) if floor == 0 else (-2.4, -0.4, du)):
            front.window(u, z, 1.0, 1.95, shutters=False, glass="glass")
            front.box((u - 0.72, -0.14, z + 2.04), (u + 0.72, 0.0, z + 2.3), "rock", name="Lintel", tile=1.0)
    # The basement's windows behind an areaway railing.
    for u in (-2.4, -0.4):
        front.window(u, 0.35, 0.9, 0.7, shutters=False, cross=False)
    for u in () if FAR else (-3.9, -2.2, -0.5, 1.0):
        front.box((u - 0.03, -1.63, 0.0), (u + 0.03, -1.57, 0.95), "iron", name="Areaway")
    for z in () if FAR else (0.12, 0.88):
        front.box((-3.9, -1.63, z), (1.03, -1.57, z + 0.06), "iron", name="Areaway")
    # The cornice: a frieze, brackets, and a deep overhang.
    front.box((-hw - 0.05, -0.12, top - 0.55), (hw + 0.05, 0.0, top - 0.1), "rock", name="Frieze", tile=1.0)
    for i in range(0 if FAR else 7):
        u = -hw + 0.4 + (w - 0.8) * i / 6
        front.box((u - 0.09, -0.45, top - 0.5), (u + 0.09, 0.0, top), "fascia", name="Bracket")
    front.box((-hw - 0.15, -0.6, top), (hw + 0.15, 0.0, top + 0.22), "fascia", name="Cornice")
    b.block((-hw, 0.0, top), (hw, d, top + 0.12), "rock", name="RoofDeck", scale=2.0)
    for x in (-hw + 0.1, hw - 0.1):
        b.block((x - 0.1, 0.0, top), (x + 0.1, d, top + 0.7), "stone", name="Parapet")
    b.block((-hw, d - 0.2, top), (hw, d, top + 0.7), "stone", name="Parapet")
    chimney(b, -hw + 0.45, d * 0.55, top, top + 1.4, w=0.6)
    chimney(b, hw - 0.45, d * 0.7, top, top + 1.2, w=0.6)
    back = Face(b, 180, (0, d, 0))
    for floor in range(3):
        z = base + floor * storey + 0.8
        for u in (-2.4, 0.0, 2.4):
            back.window(u, z, 0.9, 1.6, shutters=False)
    back.door(0.0, 1.0, 2.2, z=0.0)
    b.collide("house", (-hw - 0.1, -0.1, 0), (hw + 0.1, d + 0.1, top))
    b.collide("stoop", (sx0 - 0.2, foot, 0), (sx1 + 0.2, 0.0, base))
    b.roofs.append(((0.0, d / 2), True, (d / 2 - 0.2, hw - 0.2), top + 0.12, top + 0.13))
    b.front = (du, foot - 0.6)
    return b


# --------------------------------------------------------------------------
# A timber-framed house with a cross gable


@model
def timber_house():
    """A stone ground floor under a jettied, timber-framed upper floor, a
    cross gable over the front, and a small balcony."""
    b = house("timber_house", {"plaster": "white", "roof": "brown", "timber": "dark"},
              shutter=(0.44, 0.14, 0.12))
    w, d, g, up, jet = 8.0, 9.0, 3.0, 2.9, 0.5
    hw = w / 2
    top = g + up
    slab_walls(b, w, d, 0.0, g, mat="stone")
    front = Face(b, 0, (0, 0, 0))
    front.door(-2.3, 1.1, 2.3, glazed=True)
    if not FAR:
        bl.lantern(b, -1.35, -0.02, 2.0)
    for u in (0.2, 2.5):
        front.window(u, 0.95, 1.2, 1.3, flowers=True)
    jetty_joists(b, w, g, jet)
    slab_walls(b, w, d, g, top, y0=-jet)
    upper = Face(b, 0, (0, -jet, 0))
    upper.window(-2.5, g + 0.85, 1.0, 1.3)
    upper.window(-0.4, g + 0.85, 0.8, 1.3, shutters=False)
    upper.door(2.4, 1.0, 2.15, z=g + 0.05, glazed=True)
    upper.framing(g, top, -hw, hw, (-hw + 0.1, -1.5, 0.7, 1.6, hw - 0.1), rails=(g + 0.62,),
                  braces=((-hw + 0.2, g + 0.16, -3.2, g + 0.6), (-1.5, g + 0.16, -1.05, g + 0.6),
                          (0.7, g + 0.16, 0.3, g + 0.6), (hw - 0.2, top - 0.2, 3.2, top - 0.75)))
    # St Andrew's crosses under the upper windows.
    for u in (-2.5, -0.4):
        upper.brace(u - 0.45, g + 0.18, u + 0.45, g + 0.6, w=0.12)
        upper.brace(u + 0.45, g + 0.18, u - 0.45, g + 0.6, w=0.12)
    # The balcony before the upper door.
    b.block((1.6, -jet - 1.0, g - 0.05), (3.2, -jet, g + 0.08), "wood", name="Balcony", scale=1.0)
    for x in () if FAR else (1.65, 2.4, 3.15):
        b.beam((x - 0.04, -jet - 0.98, g + 0.08), (x + 0.04, -jet - 0.9, g + 1.0), band=DARK, name="Baluster")
    for x0, x1, y0, y1 in ((1.6, 3.2, -jet - 1.0, -jet - 0.9), (1.6, 1.7, -jet - 1.0, -jet),
                           (3.1, 3.2, -jet - 1.0, -jet)):
        b.beam((x0, y0, g + 0.95), (x1, y1, g + 1.05), band=LIGHT, name="Handrail")
    for x in () if FAR else (1.7, 3.1):
        for y in (-jet - 0.5,):
            b.beam((x - 0.04, y - 0.04, g + 0.08), (x + 0.04, y + 0.04, g + 1.0), band=DARK, name="Baluster")
    for side, rot in ((-1, -90), (1, 90)):
        f = Face(b, rot, (side * hw, d / 2, 0))
        f.window(0.0, 0.95, 1.0, 1.2, shutters=False)
        f.window(0.0, g + 0.85, 1.0, 1.2, shutters=False)
        f.framing(g, top, -(d + jet) / 2, d / 2 - jet / 2, (-2.0, 2.0), rails=(g + 0.62,))
    back = Face(b, 180, (0, d, 0))
    for u in (-2.0, 2.0):
        back.window(u, 0.95, 1.0, 1.2)
        back.window(u, g + 0.85, 1.0, 1.2)
    corner_posts(b, w, d, g, top, y0=-jet)
    # The main roof runs along the street; the cross gable faces it.
    depth = d + jet
    ridge = bl.slab_roof(b, depth, w, top + 0.02, 3.2, center=(0, (d - jet) / 2), along_x=True, over=0.5,
                         thick=0.2)
    cross = bl.slab_roof(b, 3.8, 3.2, top + 0.02, 2.3, center=(-1.5, -jet + 1.6), along_x=False, over=0.35,
                         thick=0.18)
    gable = Face(b, 0, (0, -jet - 0.01, 0))
    gable.window(-1.5, top + 0.35, 0.6, 0.9, shutters=False, cross=False)
    gable.beam((-1.59, -0.05, top), (-1.41, 0.0, cross - 0.25))
    gable.brace(-3.3, top + 0.1, -1.6, cross - 0.3)
    gable.brace(0.3, top + 0.1, -1.4, cross - 0.3)
    chimney(b, 2.4, d / 2 + 1.2, top, ridge + 0.5)
    b.collide("house", (-hw - 0.1, -0.1, 0), (hw + 0.1, d + 0.1, top))
    b.roofs.append(((0.0, (d - jet) / 2), True, (depth / 2 + 0.5, hw + 0.5), top, ridge))
    b.front = (-2.3, -1.0)
    return b


# --------------------------------------------------------------------------
# The Lantern Quarter's inn


@model
def lantern_inn():
    """A broad inn: a stone ground floor of warm lamplit windows and a
    double door, a timber-framed upper floor, a hipped roof with dormers,
    wall lanterns, and a hanging sign."""
    b = house("lantern_inn", {"plaster": "butter", "roof": "red", "timber": "dark"},
              shutter=(0.30, 0.16, 0.10))
    w, d, g, up = 10.0, 9.0, 3.2, 2.9
    hw = w / 2
    top = g + up
    slab_walls(b, w, d, 0.0, g, mat="stone")
    slab_walls(b, w, d, g, top)
    front = Face(b, 0, (0, 0, 0))
    for u in (-3.7, -1.9, 1.9, 3.7):
        front.window(u, 0.9, 1.3, 1.5, glass="lit", shutters=False)
    front.door(-0.45, 0.9, 2.5, glazed=False)
    front.door(0.45, 0.9, 2.5, glazed=False)
    front.box((-0.9, -0.02, 2.62), (0.9, 0.02, 2.95), "lit", name="Transom")
    for u in () if FAR else (-1.25, 1.25):
        bl.lantern(b, u, -0.02, 2.2)
    upper = Face(b, 0, (0, 0, 0))
    for u in (-3.6, -1.2, 1.2, 3.6):
        upper.window(u, g + 0.85, 1.0, 1.3, glass="lit" if u in (-1.2, 3.6) else "glass", flowers=True)
    upper.framing(g, top, -hw, hw, (-hw + 0.1, -2.4, 0.0, 2.4, hw - 0.1), rails=(g + 0.62,),
                  braces=((-hw + 0.2, g + 0.16, -4.6, top - 0.2), (hw - 0.2, g + 0.16, 4.6, top - 0.2),
                          (-2.4, g + 0.16, -2.0, g + 0.6), (2.4, g + 0.16, 2.0, g + 0.6)))
    front.beam((-hw, -0.08, g - 0.12), (hw, 0.0, g + 0.08))
    for side, rot in ((-1, -90), (1, 90)):
        f = Face(b, rot, (side * hw, d / 2, 0))
        for u in (-2.0, 2.0):
            f.window(u, 0.9, 1.1, 1.3, glass="lit" if u < 0 else "glass", shutters=False)
            f.window(u, g + 0.85, 1.0, 1.2)
        f.framing(g, top, -d / 2, d / 2, (0.0,), rails=(g + 0.62,))
    back = Face(b, 180, (0, d, 0))
    for u in (-3.0, 0.0, 3.0):
        back.window(u, g + 0.85, 1.0, 1.2)
    back.door(-3.0, 1.0, 2.3)
    back.window(1.5, 0.9, 1.1, 1.3, glass="lit", shutters=False)
    corner_posts(b, w, d, g, top)
    ridge = bl.hip_roof(b, w, d, top + 0.05, 3.3, center=(0, d / 2), over=0.55)
    run = d / 2 + 0.55
    zf = lambda y: top + 0.05 + 3.3 * (y + 0.55) / run  # noqa: E731
    for x in (-2.4, 2.4):
        dormer(b, x, 0.7, zf(0.7) - 0.05, w=1.5, h=1.25, depth=1.8, glass="lit" if x < 0 else "glass")
    chimney(b, -3.0, d / 2 + 0.6, top, ridge + 0.4)
    chimney(b, 3.4, d / 2 + 1.4, top, ridge + 0.2)
    if not FAR:
        bl.hanging_sign(b, hw - 0.4, 0.0, g - 0.2, rot=0, reach=1.3)
    b.collide("inn", (-hw - 0.1, -0.1, 0), (hw + 0.1, d + 0.1, top))
    b.roofs.append(((0.0, d / 2), True, (d / 2 + 0.55, (w - d) / 2 + 0.8), top + 0.05, ridge))
    b.front = (0.0, -1.0)
    return b


def main():
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if "--kit" in args:
        i = args.index("--kit")
        bl.KIT = os.path.expanduser(args[i + 1])
        del args[i:i + 2]
    out = args[0] if args else os.path.join(os.path.dirname(__file__), "..", "..", "assets", "verse",
                                            "generated", "buildings")
    global FAR
    names = args[1:] or list(HOUSES)
    for name in names:
        FAR = False
        save(HOUSES[name](), out)
        FAR = True
        save_far(HOUSES[name](), os.path.join(out, "far"))


main()
