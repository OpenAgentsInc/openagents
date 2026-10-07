"""Build whole village buildings from Quaternius's Medieval Village MegaKit.

Run headless:
    Blender -b --factory-startup --python scripts/blender/buildings.py -- \
        OUT_DIR [NAME ...] [--kit KIT_DIR]

Each building is assembled in Blender from the kit's own 2 m wall pieces,
windows, doors, roofs, dormers, balconies, and supports, plus a few generated
solids (beams, plinths, a round tower, a clock turret, signs) textured with the
kit's images. Every building is written to `OUT_DIR/<name>.glb` with a
`<name>.footprint.json` of collision boxes beside it.

Frame: 1 unit = 1 m. In Blender the front faces -Y; the glTF export turns that
into +Z forward with +Y up. The origin is on the ground at the center of the
frontmost ground-floor wall. KIT_DIR defaults to
`~/Downloads/Medieval Village MegaKit[Standard]` (CC0 1.0, Quaternius).

The kit's roof tiles, plaster, and timber are recolored per building from the
kit's base-color images, so each glb carries its own small textures.
"""

import json
import math
import os
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(__file__))
import coplanar  # noqa: E402

KIT = os.path.expanduser("~/Downloads/Medieval Village MegaKit[Standard]")
STOREY = 3.0  # Floor-to-floor height of the kit's walls.
WALL_TOP = 0.12  # The wall's top beam rises this far above the next floor.

# Colors in sRGB. Roof and plaster colors replace the texture's hue and keep
# its light and dark; timber values scale the wood.
PLASTER = {
    "cream": (0.90, 0.82, 0.64),
    "ochre": (0.86, 0.66, 0.38),
    "rose": (0.88, 0.70, 0.66),
    "white": (0.95, 0.93, 0.88),
    "sage": (0.76, 0.81, 0.66),
    "sky": (0.74, 0.82, 0.88),
    "butter": (0.95, 0.86, 0.56),
    "terracotta": (0.82, 0.54, 0.42),
    "lilac": (0.80, 0.74, 0.86),
}
ROOF = {
    "red": None,
    "brown": (0.52, 0.33, 0.22),
    "slate": (0.33, 0.37, 0.44),
    "green": (0.42, 0.50, 0.42),
    "teal": (0.26, 0.46, 0.46),
    "charcoal": (0.29, 0.29, 0.31),
    "ochre": (0.74, 0.54, 0.30),
    "plum": (0.47, 0.29, 0.33),
}
TIMBER = {"light": 1.0, "mid": 0.75, "dark": 0.52}

# The saved models whose faces of two materials overlap in one plane
# (`coplanar.py`); `main` fails when any do.
FLICKERS = []

KIT_MATERIALS = {
    "MI_WoodTrim": "wood",
    "MI_WoodTrim_Wear": "wood",
    "MI_Plaster": "plaster",
    "MI_RoundTiles": "tiles",
    "MI_Brick": "brick",
    "MI_RockTrim": "rock",
    "MI_UnevenBrick": "stone",
    "MI_RedBrick": "redbrick",
    "MI_MetalOrnaments": "metal",
    "MI_WindowGlass": "glass",
}

# The wood atlas's bands, as (v_low, v_high) in Blender UV space.
LIGHT_WOOD = (0.71, 0.99)
DARK_WOOD = (0.39, 0.68)
GREY_STONE = (0.06, 0.19)


# --------------------------------------------------------------------------
# Textures and materials


def _pixels(path, size):
    img = bpy.data.images.load(path, check_existing=True)
    if img.size[0] != size:
        img = img.copy()
        img.scale(size, size)
    px = np.empty(size * size * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    return px.reshape(size, size, 4)


def _image(name, px):
    h, w = px.shape[:2]
    img = bpy.data.images.new(name, w, h, alpha=False)
    img.pixels.foreach_set(np.clip(px, 0, 1).astype(np.float32).ravel())
    img.pack()
    return img


def _recolor(px, color, keep_green=False):
    """Map the texture's luminance onto `color`, keeping its detail."""
    lum = px[..., 0] * 0.299 + px[..., 1] * 0.587 + px[..., 2] * 0.114
    rel = lum / max(float(lum.mean()), 1e-3)
    out = px.copy()
    for c in range(3):
        out[..., c] = color[c] * rel
    if keep_green:
        moss = (px[..., 1] > px[..., 0] * 0.95)[..., None]
        out[..., :3] = np.where(moss, px[..., :3] * 0.85, out[..., :3])
    return out


def _darken(px, k):
    lum = (px[..., 0] * 0.299 + px[..., 1] * 0.587 + px[..., 2] * 0.114)[..., None]
    out = px.copy()
    sat = 0.55 + 0.45 * k  # Darker timber is also less saturated.
    out[..., :3] = (lum + (px[..., :3] - lum) * sat) * k
    return out


def _material(name, image=None, color=(0.8, 0.8, 0.8), rough=0.85, emit=None):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    mat.use_backface_culling = False
    nodes = mat.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = rough
    bsdf.inputs["Metallic"].default_value = 0.0
    if image is not None:
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = image
        mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    else:
        bsdf.inputs["Base Color"].default_value = (*_linear(color), 1.0)
    if emit is not None:
        bsdf.inputs["Emission Color"].default_value = (*_linear(emit), 1.0)
        bsdf.inputs["Emission Strength"].default_value = 1.5
    return mat


def _clear_glass():
    """A pale, see-through glass for the greenhouse: glTF BLEND."""
    mat = bpy.data.materials.new("GreenhouseGlass")
    mat.use_nodes = True
    mat.use_backface_culling = False
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*_linear((0.78, 0.9, 0.88)), 1.0)
    bsdf.inputs["Roughness"].default_value = 0.1
    bsdf.inputs["Alpha"].default_value = 0.32
    if hasattr(mat, "surface_render_method"):
        mat.surface_render_method = "BLENDED"
    if hasattr(mat, "blend_method"):
        mat.blend_method = "BLEND"
    return mat


def _linear(c):
    return tuple(((x + 0.055) / 1.055) ** 2.4 if x > 0.04045 else x / 12.92 for x in c)


def make_materials(scheme):
    """Build this building's materials from the kit's base-color images."""
    tex = os.path.join(KIT, "glTF")
    plaster, roof, timber = scheme["plaster"], scheme["roof"], scheme["timber"]
    px = _pixels(os.path.join(tex, "T_Plaster_BaseColor.png"), 512)
    img_plaster = _image(f"T_Plaster_{plaster}", _recolor(px, PLASTER[plaster]))
    px = _pixels(os.path.join(tex, "T_RoundTiles_BaseColor.png"), 512)
    if ROOF[roof] is not None:
        px = _recolor(px, ROOF[roof], keep_green=True)
    img_tiles = _image(f"T_RoundTiles_{roof}", px)
    px = _pixels(os.path.join(tex, "T_WoodTrim_BaseColor.png"), 1024)
    if TIMBER[timber] != 1.0:
        px = _darken(px, TIMBER[timber])
    img_wood = _image(f"T_WoodTrim_{timber}", px)
    plain = {}
    for key, file in [
        ("brick", "T_Brick_BaseColor.png"),
        ("rock", "T_RockTrim_BaseColor.png"),
        ("stone", "T_UnevenBrick_BaseColor.png"),
        ("metal", "T_MetalOrnaments_BaseColor.png"),
    ]:
        plain[key] = _image(file[:-14], _pixels(os.path.join(tex, file), 512))
    mats = {
        "plaster": _material(f"Plaster_{plaster}", img_plaster),
        "tiles": _material(f"Tiles_{roof}", img_tiles, rough=0.7),
        "wood": _material(f"Wood_{timber}", img_wood),
        "brick": _material("Brick", plain["brick"]),
        "rock": _material("RockTrim", plain["rock"]),
        "stone": _material("Stone", plain["stone"]),
        "metal": _material("Metal", plain["metal"], rough=0.5),
        "glass": _material("Glass", color=(0.20, 0.25, 0.31), rough=0.2),
        "lamp": _material("LampGlass", color=(1.0, 0.78, 0.42), emit=(1.0, 0.7, 0.35)),
        "paint": _material("SignPaint", color=(0.80, 0.62, 0.20), rough=0.6),
        "cloth": _material("Cloth", color=(0.62, 0.20, 0.16), rough=0.9),
        "cloth2": _material("Cloth2", color=(0.88, 0.82, 0.68), rough=0.9),
        "clock": _material("ClockFace", color=(0.93, 0.91, 0.84), rough=0.6),
        "iron": _material("Iron", color=(0.12, 0.12, 0.13), rough=0.5),
        "cloth3": _material("ClothGreen", color=(0.20, 0.42, 0.30), rough=0.9),
        "cloth4": _material("ClothBlue", color=(0.18, 0.30, 0.55), rough=0.9),
        "leaf": _material("Leaf", color=(0.20, 0.38, 0.12), rough=0.9),
        "thatch": _material("Thatch", color=(0.66, 0.52, 0.27), rough=1.0),
        "thatch_dark": _material("ThatchDark", color=(0.50, 0.38, 0.18), rough=1.0),
        "boards": _material("Boards", color=(0.34, 0.22, 0.13), rough=0.9),
        "coals": _material("Coals", color=(1.0, 0.45, 0.12), emit=(1.0, 0.4, 0.1)),
        "white_paint": _material("WhitePaint", color=(0.92, 0.91, 0.86), rough=0.6),
        "barn": _material("BarnRed", color=(0.50, 0.11, 0.07), rough=0.9),
        "glass_clear": _clear_glass(),
    }
    mats["redbrick"] = mats["stone"]
    return mats


# --------------------------------------------------------------------------
# Kit pieces


class Kit:
    """Imports kit pieces once each and hands out linked copies."""

    def __init__(self, mats):
        self.mats = mats
        self.meshes = {}

    def mesh(self, name):
        if name in self.meshes:
            return self.meshes[name]
        if name.endswith("+upper"):
            # Upper floors: the brick plinth band becomes plaster.
            mesh = self.mesh(name[:-6]).copy()
            for i, mat in enumerate(mesh.materials):
                if mat == self.mats["brick"]:
                    mesh.materials[i] = self.mats["plaster"]
            self.meshes[name] = mesh
            return mesh
        before = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=os.path.join(KIT, "glTF", name + ".gltf"))
        new = [o for o in bpy.data.objects if o not in before]
        parts = [o for o in new if o.type == "MESH"]
        bm = bmesh.new()
        slots = []
        for o in parts:
            o.data.calc_loop_triangles()
            m = o.data.copy()
            m.transform(o.matrix_world)
            if o.matrix_world.determinant() < 0:
                m.flip_normals()
            # Keep only the first UV map; the second carries wear masks.
            while len(m.uv_layers) > 1:
                m.uv_layers.remove(m.uv_layers[-1])
            remap = []
            for mat in m.materials:
                key = KIT_MATERIALS[mat.name.split(".")[0]] if mat else "wood"
                if key not in slots:
                    slots.append(key)
                remap.append(slots.index(key))
            for p in m.polygons:
                p.material_index = remap[p.material_index] if remap else 0
            bm.from_mesh(m)
            bpy.data.meshes.remove(m)
        mesh = bpy.data.meshes.new("kit_" + name)
        bm.to_mesh(mesh)
        bm.free()
        ratio = next((r for prefix, r in LIGHTER.items() if name.startswith(prefix)), None)
        if ratio is not None and os.environ.get("NO_LIGHTER") is None:
            mesh = lighter(mesh, ratio)
        for key in slots:
            mesh.materials.append(self.mats[key])
        for o in new:
            bpy.data.objects.remove(o, do_unlink=True)
        self.meshes[name] = mesh
        return mesh


# The kit's densest pieces, thinned by edge collapse: round tiles and arched
# windows keep their silhouettes with a third fewer triangles.
LIGHTER = {
    "Roof_RoundTiles_": 0.6,
    "Window_Wide_Round1": 0.75,
    "Window_Thin_Round1": 0.75,
    "Roof_Dormer_RoundTile": 0.65,
}


def lighter(mesh, ratio):
    obj = bpy.data.objects.new("lighter", mesh)
    bpy.context.scene.collection.objects.link(obj)
    mod = obj.modifiers.new("decimate", "DECIMATE")
    mod.decimate_type = "COLLAPSE"
    mod.ratio = ratio
    mod.use_collapse_triangulate = True
    dg = bpy.context.evaluated_depsgraph_get()
    out = bpy.data.meshes.new_from_object(obj.evaluated_get(dg))
    out.name = mesh.name
    for i, mat in enumerate(mesh.materials):
        out.materials[i] = mat
    bpy.data.objects.remove(obj)
    return out


# How far a storey's wall runs stop short of its corners, m.
INSET = 0.01


def inset(tokens):
    """Wall bay `tokens` narrowed by the same share to span `INSET` less at
    each end."""
    items = [t if isinstance(t, tuple) else (t, 2.0) for t in tokens]
    total = sum(width for _, width in items)
    k = (total - 2 * INSET) / total
    return [(t, width * k) for t, width in items]


class Building:
    """One building under construction, plus its collision boxes."""

    def __init__(self, name, scheme):
        bpy.ops.wm.read_factory_settings(use_empty=True)
        self.name = name
        self.mats = make_materials(scheme)
        self.kit = Kit(self.mats)
        self.col = bpy.context.scene.collection
        self.boxes = []
        # Landing roofs, the walk's end outside the door, and a point past
        # an open doorway, in Blender x and y, for the layout's model data.
        self.roofs = []
        self.front = None
        self.inside = None
        self.count = 0

    # -- placing ---------------------------------------------------------

    def put(self, piece, loc, rot=0.0, scale=(1, 1, 1)):
        """Place a kit piece; `rot` is degrees about Z (0 faces -Y)."""
        mesh = self.kit.mesh(piece)
        self.count += 1
        obj = bpy.data.objects.new(f"{piece}.{self.count:03d}", mesh)
        obj.location = Vector(loc)
        obj.rotation_euler = (0, 0, math.radians(rot))
        obj.scale = scale
        self.col.objects.link(obj)
        return obj

    def solid(self, name, mesh):
        self.count += 1
        obj = bpy.data.objects.new(f"{name}.{self.count:03d}", mesh)
        self.col.objects.link(obj)
        return obj

    def collide(self, name, lo, hi):
        """Record an axis-aligned collision box in Blender coordinates."""
        self.boxes.append((name, Vector(lo), Vector(hi)))

    # -- walls -----------------------------------------------------------

    def run(self, tokens, start, rot, z=0.0, bay=2.0, height=STOREY, stone=False):
        """Place a run of wall bays from `start`, left to right seen outside.

        `tokens` is a string or list; each item is one bay (see `bay_pieces`),
        or a (token, width) pair to override the bay width.
        """
        a = math.radians(rot)
        along = Vector((math.cos(a), math.sin(a), 0))
        at = Vector(start)
        for tok in tokens:
            tok, width = tok if isinstance(tok, tuple) else (tok, bay)
            center = at + along * (width / 2)
            center.z = z
            self.bay(tok, center, rot, width / 2.0, height / STOREY, stone)
            at = at + along * width

    def bay(self, tok, c, rot, sx, sz, stone):
        s = (sx, 1, sz)
        if tok == "_":
            return
        wall = BAYS[tok]
        if stone and wall[0] in STONE:
            wall = (STONE[wall[0]],) + wall[1:]
        for piece in wall:
            if piece.startswith("Door_"):
                self.door(piece, c, rot, sx, sz)
            elif c.z > 0.5 and piece.startswith("Wall_Plaster"):
                self.put(piece + "+upper", c, rot, s)
            else:
                self.put(piece, c, rot, s)

    def door(self, piece, c, rot, sx, sz):
        """A closed door leaf, or a pair when the bay is wide."""
        a = math.radians(rot)
        along = Vector((math.cos(a), math.sin(a), 0))
        inward = Vector((-math.sin(a), math.cos(a), 0))
        if sx > 1.2:
            half = 0.6 * sx
            k = half / 1.12
            self.put(piece, c - along * half + inward * 0.02, rot, (k, 1, sz))
            self.put(piece, c + along * half + inward * 0.02, rot + 180, (k, 1, sz))
        else:
            self.put(piece, c - along * 0.55 * sx + inward * 0.02, rot, (sx, 1, sz))

    def corners(self, pts, z=0.0, height=STOREY, piece="Corner_Exterior_Wood"):
        for x, y in pts:
            self.put(piece, (x, y, z), 0, (1, 1, height / STOREY))

    def box_storey(self, w, d, z, fronts, sides=None, back=None, jetty=0.0,
                   height=STOREY, stone=False, corner="Corner_Exterior_Wood",
                   x0=None, skip=()):
        """Four walls of one storey: front at y=-jetty, back at y=d.

        Width `w` must be a multiple of 2; side bays stretch to fit the depth.
        """
        x0 = -w / 2 if x0 is None else x0
        y0 = -jetty
        depth = d + jetty
        n_side = max(1, round(depth / 2.0))
        # Each run stops `INSET` short of the corners, so its end faces sit
        # inside the walls it meets instead of in their outer faces.
        side_bay = (depth - 2 * INSET) / n_side
        sides = sides or "P" * n_side
        left, right = (sides, sides) if isinstance(sides, str) else sides
        # Pad or trim each side to its bay count.
        left, right = [list(s) + ["P"] * n_side for s in (left, right)]
        back = back or "P" * int(w / 2)
        if "front" not in skip:
            self.run(inset(fronts), (x0 + INSET, y0, z), 0, z, height=height, stone=stone)
        if "back" not in skip:
            self.run(inset(back), (x0 + w - INSET, d, z), 180, z, height=height, stone=stone)
        if "left" not in skip:
            self.run(left[:n_side], (x0, d - INSET, z), -90, z, bay=side_bay, height=height, stone=stone)
        if "right" not in skip:
            self.run(right[:n_side], (x0 + w, y0 + INSET, z), 90, z, bay=side_bay, height=height, stone=stone)
        pts = [(x0, y0), (x0 + w, y0), (x0, d), (x0 + w, d)]
        self.corners(pts, z, height, corner)

    def jetty(self, w, z, out, x0=None, y=0.0):
        """The underside of a jettied floor: joist ends, soffit, and beams."""
        x0 = -w / 2 if x0 is None else x0
        n = max(2, int(round(w / 1.7)))
        for i in range(n + 1):
            # The end joists stand 3 cm in from the side walls, so their
            # sides don't share the walls' planes.
            x = x0 + 0.13 + (w - 0.26) * i / n
            self.put("Roof_Support2", (x, -0.09 + y, z + 0.02), 0, (1, out / 0.69, 1))
        # Soffit boards between the wall and the jettied floor.
        for i in range(int(w / 2)):
            self.put("Floor_WoodDark", (x0 + 1 + 2 * i, -out / 2 + y, z - 0.03), 0,
                     (1, (out + 0.1) / 2, 1))
        self.beam((x0 - 0.05, -out - 0.06 + y, z - 0.14), (x0 + w + 0.05, -out + 0.16 + y, z + 0.02))

    # -- generated solids -------------------------------------------------

    def beam(self, lo, hi, band=LIGHT_WOOD, mat="wood", name="Beam", skip=""):
        mesh = box_mesh(name, lo, hi, self.mats[mat], band=band, skip=skip)
        return self.solid(name, mesh)

    def block(self, lo, hi, mat, name="Block", scale=2.0, skip=""):
        mesh = box_mesh(name, lo, hi, self.mats[mat], tile=scale, skip=skip)
        return self.solid(name, mesh)

    # -- roofs -----------------------------------------------------------

    def gable_roof(self, span, length, z, center=(0, 0), along_x=False, ends=(True, True),
                   size=None, gable=True, piece_len=None):
        """A kit round-tile roof with its plaster gables.

        The kit's roofs run their ridge along Y; `along_x` turns the ridge
        parallel to the front. `span` and `length` stretch the nearest piece.
        """
        k = size or {4: 4, 6: 6, 8: 8}[min((4, 6, 8), key=lambda s: abs(s - span))]
        lengths = ROOF_LENGTHS[k]
        lpiece = piece_len or min(lengths, key=lambda l: abs(l - length))
        rot = -90 if along_x else 0
        sx, sy = span / k, length / lpiece
        cx, cy = center
        roof = self.put(f"Roof_RoundTiles_{k}x{lpiece}", (cx, cy, z), rot, (sx, sy, 1))
        if gable:
            for end, sign in zip(ends, (-1, 1)):
                if not end:
                    continue
                if along_x:
                    at = (cx + sign * length / 2, cy, z)
                    r = -90 if sign < 0 else 90
                else:
                    at = (cx, cy + sign * length / 2, z)
                    r = 0 if sign < 0 else 180
                self.put(f"Roof_Front_Brick{k}", at, r, (sx, 1, 1))
        return roof

    def roof_z(self, x, y, top=40.0):
        """Height of the highest surface under (x, y), by ray cast."""
        dg = bpy.context.evaluated_depsgraph_get()
        bpy.context.view_layer.update()
        hit, loc, *_ = bpy.context.scene.ray_cast(dg, Vector((x, y, top)), Vector((0, 0, -1)))
        return loc.z if hit else None

    def chimney(self, x, y, piece="Prop_Chimney", above=1.4):
        z = self.roof_z(x, y)
        # The kit chimney is 3.18 m; sink it so `above` metres show.
        self.put(piece, (x, y, z + above - 3.0), 0)

    def dormer(self, x, y_eave, rot=-90, scale=1.0):
        """A kit dormer on a roof slope that falls toward -Y (rot -90)."""
        # Find where the slope is about 1.5 m above the eave line.
        o = self.put("Roof_Dormer_RoundTile", (0, 0, -100), rot, (scale,) * 3)
        a = math.radians(rot)
        face = Vector((math.cos(a), math.sin(a), 0))  # Dormer's +X, its front.
        p = Vector((x, y_eave, 0))
        z = self.roof_z(*(p - face * 0.6).xy)
        o.location = Vector((p.x, p.y, z - 0.85 * scale)) - face * 0.6
        return o

    # -- output ----------------------------------------------------------

    def triangles(self):
        n = 0
        for o in self.col.objects:
            if o.type == "MESH":
                o.data.calc_loop_triangles()
                n += len(o.data.loop_triangles)
        return n

    def save(self, out_dir):
        os.makedirs(out_dir, exist_ok=True)
        path = os.path.join(out_dir, self.name + ".glb")
        parts = [o for o in self.col.objects if o.type == "MESH"]
        drop_ground_faces(parts)
        # A model joined into one object (`town_houses.py`) has no parts to name.
        if coplanar.parts_wanted() and len(parts) > 1:
            coplanar.report_parts(self.name, parts)
        bpy.ops.export_scene.gltf(
            filepath=path,
            export_format="GLB",
            export_yup=True,
            export_apply=True,
            export_image_format="JPEG",
            export_jpeg_quality=88,
            export_tangents=False,
            export_cameras=False,
            export_lights=False,
            export_extras=False,
        )
        boxes = []
        for name, lo, hi in self.boxes:
            # Blender (x, y, z) is glTF (x, z, -y).
            c, h = (lo + hi) / 2, (hi - lo) / 2
            boxes.append({
                "name": name,
                "center": [round(c.x, 3), round(c.z, 3), round(-c.y, 3)],
                "half_extents": [round(abs(h.x), 3), round(abs(h.z), 3), round(abs(h.y), 3)],
            })
        footprint = {
            "model": self.name + ".glb",
            "frame": "glTF: 1 unit = 1 m, +Y up, +Z out of the front door, "
                     "origin on the ground at the center of the front wall",
            "triangles": self.triangles(),
            "boxes": boxes,
            # In glTF x and z, as `layout::generated::Model` takes them.
            "roofs": [
                {"center": [round(cx, 3), round(-cy, 3)], "slopes_z": along_x,
                 "half": [round(h0, 3), round(h1, 3)], "eave": round(eave, 3), "ridge": round(ridge, 3)}
                for (cx, cy), along_x, (h0, h1), eave, ridge in self.roofs
            ],
            "front": None if self.front is None else [round(self.front[0], 3), round(-self.front[1], 3)],
            "inside": None if self.inside is None else [round(self.inside[0], 3), round(-self.inside[1], 3)],
        }
        with open(os.path.join(out_dir, self.name + ".footprint.json"), "w") as f:
            json.dump(footprint, f, indent=2)
            f.write("\n")
        tally = {}
        for o in self.col.objects:
            if o.type == "MESH":
                key = o.name.rsplit(".", 1)[0]
                tally[key] = tally.get(key, 0) + len(o.data.loop_triangles)
        top = sorted(tally.items(), key=lambda kv: -kv[1])[:8]
        print("  heaviest:", ", ".join(f"{k} {v}" for k, v in top))
        print(f"BUILT {self.name} triangles={footprint['triangles']} boxes={len(boxes)} -> {path}")
        flickers(path)


def flickers(path):
    """Check a saved model for faces of two materials in one plane, which
    z-fight: the renderer can't tell which is in front, so the surface
    flickers as the camera moves."""
    if not coplanar.report(path):
        FLICKERS.append(path)


def fail_on_flickers():
    if FLICKERS:
        print(f"COPLANAR faces in {len(FLICKERS)} models; offset or remove one face of each pair")
        sys.exit(1)


def drop_ground_faces(objs, height=0.002):
    """Delete the faces of `objs` that face down on the ground plane. The
    ground hides them, and where two parts stand side by side, their
    bottoms share the plane and would z-fight from below."""
    for o in objs:
        m = o.matrix_world
        rot = m.to_3x3()
        down = [p.index for p in o.data.polygons
                if (rot @ p.normal).normalized().z < -0.99
                and all((m @ o.data.vertices[v].co).z < height for v in p.vertices)]
        if not down:
            continue
        # Kit pieces share their mesh; give this one its own copy.
        if o.data.users > 1:
            o.data = o.data.copy()
        bm = bmesh.new()
        bm.from_mesh(o.data)
        bm.faces.ensure_lookup_table()
        bmesh.ops.delete(bm, geom=[bm.faces[i] for i in down], context="FACES")
        bm.to_mesh(o.data)
        bm.free()
        o.data.update()


ROOF_LENGTHS = {4: [4, 6, 8], 6: [4, 6, 8, 10, 12, 14], 8: [8, 10, 12, 14]}

# Wall bay tokens and the kit pieces each places on a 2 m bay.
BAYS = {
    "P": ("Wall_Plaster_Straight",),
    "B": ("Wall_Plaster_Straight_Base",),
    "L": ("Wall_Plaster_Straight_L",),
    "R": ("Wall_Plaster_Straight_R",),
    "G": ("Wall_Plaster_WoodGrid",),
    "W": ("Wall_Plaster_Window_Wide_Round", "Window_Wide_Round1", "WindowShutters_Wide_Round_Open"),
    "w": ("Wall_Plaster_Window_Wide_Round", "Window_Wide_Round1"),
    "F": ("Wall_Plaster_Window_Wide_Flat", "Window_Wide_Flat1"),
    "f": ("Wall_Plaster_Window_Wide_Flat2", "Window_Wide_Flat1"),
    "S": ("Wall_Plaster_Window_Wide_Flat", "Window_Wide_Flat1", "WindowShutters_Wide_Flat_Open"),
    "T": ("Wall_Plaster_Window_Thin_Round", "Window_Thin_Round1"),
    "t": ("Wall_Plaster_Window_Thin_Round", "Window_Thin_Round1", "WindowShutters_Thin_Round_Open"),
    "D": ("Wall_Plaster_Door_Round", "DoorFrame_Round_WoodDark", "Door_1_Round"),
    "E": ("Wall_Plaster_Door_Round", "DoorFrame_Round_WoodDark", "Door_1_Round"),
    "Q": ("Wall_Plaster_Door_Flat", "DoorFrame_Flat_WoodDark", "Door_8_Flat"),
    "A": ("Wall_Arch",),
}
# Stone ground floors swap the plaster wall for the uneven-brick one.
STONE = {
    "Wall_Plaster_Straight": "Wall_UnevenBrick_Straight",
    "Wall_Plaster_Straight_Base": "Wall_UnevenBrick_Straight",
    "Wall_Plaster_Straight_L": "Wall_UnevenBrick_Straight",
    "Wall_Plaster_Straight_R": "Wall_UnevenBrick_Straight",
    "Wall_Plaster_Window_Wide_Round": "Wall_UnevenBrick_Window_Wide_Round",
    "Wall_Plaster_Window_Wide_Flat": "Wall_UnevenBrick_Window_Wide_Flat",
    "Wall_Plaster_Window_Thin_Round": "Wall_UnevenBrick_Window_Thin_Round",
    "Wall_Plaster_Door_Round": "Wall_UnevenBrick_Door_Round",
    "Wall_Plaster_Door_Flat": "Wall_UnevenBrick_Door_Flat",
}


# --------------------------------------------------------------------------
# Generated meshes


def box_mesh(name, lo, hi, mat, band=None, tile=2.0, xf=None, skip=""):
    """A box with UVs: along a wood band, or tiled at `tile` metres.

    `xf` moves the finished box, for boxes built in a wall's local frame.
    `skip` names the sides to leave out, such as "-z +x", in the box's own
    axes: a side that rests on another solid is hidden, and its face would
    share a plane with that solid's.
    """
    lo, hi = Vector(lo), Vector(hi)
    size = hi - lo
    long_axis = max(range(3), key=lambda i: size[i])
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    corners = [Vector((x, y, z)) for x in (lo.x, hi.x) for y in (lo.y, hi.y) for z in (lo.z, hi.z)]
    verts = [bm.verts.new(c) for c in corners]
    faces = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
    normals = [0, 0, 1, 1, 2, 2]
    sides = ["-x", "+x", "-y", "+y", "-z", "+z"]
    for f, axis, side in zip(faces, normals, sides):
        if side in skip.split():
            continue
        face = bm.faces.new([verts[i] for i in f])
        # The face's two in-plane axes.
        plane = [i for i in range(3) if i != axis]
        if band is not None:
            ua = long_axis if long_axis in plane else plane[0]
            va = plane[1] if ua == plane[0] else plane[0]
        else:
            ua, va = (plane[0], plane[1]) if axis == 2 else ((0 if axis == 1 else 1), 2)
        for loop in face.loops:
            co = loop.vert.co
            if band is not None:
                u = (co[ua] - lo[ua]) / 2.0
                t = (co[va] - lo[va]) / max(size[va], 1e-6)
                v = band[0] + (band[1] - band[0]) * t
            else:
                u, v = co[ua] / tile, co[va] / tile
            loop[uv].uv = (u, v)
    if xf is not None:
        bmesh.ops.transform(bm, matrix=xf, verts=bm.verts)
    bm.normal_update()
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(mat)
    return mesh


def quad_mesh(name, quads, mat, xf=None, tile=1.0):
    """Loose quads, UV mapped by their own edges."""
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    for q in quads:
        f = bm.faces.new([bm.verts.new(p) for p in q])
        for loop, st in zip(f.loops, [(0, 0), (1, 0), (1, 1), (0, 1)]):
            loop[uv].uv = st
    if xf is not None:
        bmesh.ops.transform(bm, matrix=xf, verts=bm.verts)
    bm.normal_update()
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(mat)
    return mesh


def prism_mesh(name, center, radius, z0, z1, sides, mat, tile=2.0, rot=0.0, band=None):
    """An upright prism (a round tower at enough sides), UV wrapped.

    With `band`, V spans that band of the wood atlas from bottom to top.
    """
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    cx, cy = center
    ring0, ring1 = [], []
    for i in range(sides + 1):
        a = rot + 2 * math.pi * i / sides
        p = (cx + radius * math.cos(a), cy + radius * math.sin(a))
        ring0.append(bm.verts.new((*p, z0)))
        ring1.append(bm.verts.new((*p, z1)))
    circ = 2 * math.pi * radius
    for i in range(sides):
        f = bm.faces.new([ring0[i], ring0[i + 1], ring1[i + 1], ring1[i]])
        for loop, (s, h) in zip(f.loops, [(i, z0), (i + 1, z0), (i + 1, z1), (i, z1)]):
            if band is None:
                loop[uv].uv = (circ * s / sides / tile, h / tile)
            else:
                t = (h - z0) / (z1 - z0)
                loop[uv].uv = (circ * s / sides / 2.0, band[0] + (band[1] - band[0]) * t)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    bm.normal_update()
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(mat)
    return mesh


def cone_mesh(name, center, radius, z0, z1, sides, mat, tile=(2.1, 1.8), rings=4, rot=0.0,
              flare=0.0):
    """A cone or pyramid roof, UV mapped down the slope.

    `flare` bends the lowest ring outward and down, like a bell-cast eave.
    """
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    cx, cy = center
    slope = math.hypot(radius, z1 - z0)
    grid = []
    for r in range(rings + 1):
        t = r / rings  # 0 at the eave, 1 at the apex.
        rad = radius * (1 - t)
        z = z0 + (z1 - z0) * t
        if r == 0:
            rad += flare
            z -= flare * 0.6
        row = []
        for i in range(sides + 1):
            a = rot + 2 * math.pi * i / sides
            row.append(bm.verts.new((cx + rad * math.cos(a), cy + rad * math.sin(a), z)))
        grid.append(row)
    circ = 2 * math.pi * radius
    for r in range(rings):
        for i in range(sides):
            quad = [grid[r][i], grid[r][i + 1], grid[r + 1][i + 1], grid[r + 1][i]]
            f = bm.faces.new(quad)
            for loop, (s, rr) in zip(f.loops, [(i, r), (i + 1, r), (i + 1, r + 1), (i, r + 1)]):
                u = circ * s / sides / tile[0]
                v = -slope * rr / rings / tile[1]
                loop[uv].uv = (u, v)
    bm.normal_update()
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(mat)
    return mesh


def disc_mesh(name, center, radius, normal_rot, mat, sides=16, depth=0.04):
    """A thin round plate facing -Y, rotated by `normal_rot` degrees about Z."""
    bm = bmesh.new()
    bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=sides,
                          radius1=radius, radius2=radius, depth=depth)
    uv = bm.loops.layers.uv.new("UVMap")
    for f in bm.faces:
        for loop in f.loops:
            loop[uv].uv = (0.5 + loop.vert.co.x / radius / 2, 0.5 + loop.vert.co.y / radius / 2)
    rot = Matrix.Rotation(math.radians(normal_rot), 4, "Z") @ Matrix.Rotation(math.pi / 2, 4, "X")
    bmesh.ops.transform(bm, matrix=Matrix.Translation(Vector(center)) @ rot, verts=bm.verts)
    bm.normal_update()
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(mat)
    return mesh


# --------------------------------------------------------------------------
# Details


def frame(rot, origin):
    """A matrix from a wall's local frame (x along, -y outward) to the world."""
    return Matrix.Translation(Vector(origin)) @ Matrix.Rotation(math.radians(rot), 4, "Z")


def lantern(b, x, y, z, rot=0):
    """A wall lantern on an iron arm, lit."""
    m = frame(rot, (x, y, z))
    b.solid("LanternArm", box_mesh("LanternArm", (-0.025, -0.42, 0.28), (0.025, 0.0, 0.33), b.mats["iron"], xf=m))
    b.solid("LanternHook", box_mesh("LanternHook", (-0.015, -0.36, 0.2), (0.015, -0.33, 0.29), b.mats["iron"], xf=m))
    b.solid("LanternCap", box_mesh("LanternCap", (-0.14, -0.49, 0.14), (0.14, -0.21, 0.2), b.mats["iron"], xf=m))
    b.solid("LanternGlass", box_mesh("LanternGlass", (-0.1, -0.45, -0.18), (0.1, -0.25, 0.14), b.mats["lamp"], xf=m))
    b.solid("LanternBase", box_mesh("LanternBase", (-0.12, -0.47, -0.24), (0.12, -0.23, -0.18), b.mats["iron"], xf=m))


def hanging_sign(b, x, y, z, rot=0, reach=1.5):
    """A carved board hanging from a beam arm that leaves the wall at height z."""
    m = frame(rot, (x, y, z))
    b.solid("SignArm", box_mesh("SignArm", (-0.07, -reach, -0.08), (0.07, 0.0, 0.08), b.mats["wood"], band=DARK_WOOD, xf=m))
    b.put("Roof_Support2", m @ Vector((0, 0, -0.06)), rot, (0.8, 0.6, 0.9))
    for yy in (-0.45, -reach + 0.2):
        b.solid("SignChain", box_mesh("SignChain", (-0.015, yy - 0.015, -0.42), (0.015, yy + 0.015, -0.08), b.mats["iron"], xf=m))
    b.solid("SignBoard", box_mesh("SignBoard", (-0.05, -reach + 0.05, -1.25), (0.05, -0.3, -0.42), b.mats["wood"], band=DARK_WOOD, xf=m))
    b.solid("SignFrame", box_mesh("SignFrame", (-0.06, -reach + 0.02, -0.48), (0.06, -0.27, -0.4), b.mats["wood"], band=LIGHT_WOOD, xf=m))
    emblem = Vector((0, -(reach + 0.25) / 2, -0.85))
    b.solid("SignEmblem", disc_mesh("SignEmblem", m @ emblem, 0.27, rot + 90, b.mats["paint"], depth=0.13))


def awning(b, x0, x1, y, z, rot=0, depth=1.3, drop=0.55):
    """A striped cloth awning on iron arms, from wall height z outward."""
    m = frame(rot, (0, 0, 0))
    n = max(2, int(round((x1 - x0) / 0.5)))
    quads = ([], [])
    for i in range(n):
        a, c = x0 + (x1 - x0) * i / n, x0 + (x1 - x0) * (i + 1) / n
        slope = [(a, y, z), (c, y, z), (c, y - depth, z - drop), (a, y - depth, z - drop)]
        hang = [(a, y - depth, z - drop), (c, y - depth, z - drop),
                (c, y - depth, z - drop - 0.28), (a, y - depth, z - drop - 0.28)]
        quads[i % 2].extend([slope, hang])
    for k, mat in enumerate(("cloth", "cloth2")):
        b.solid("Awning", quad_mesh("Awning", quads[k], b.mats[mat], xf=m))
    b.solid("AwningRod", box_mesh("AwningRod", (x0, y - depth - 0.03, z - drop - 0.03), (x1, y - depth + 0.03, z - drop + 0.03), b.mats["iron"], xf=m))
    for xx in (x0 + 0.06, x1 - 0.06):
        b.solid("AwningArm", quad_mesh("AwningArm", [[(xx - 0.02, y, z - 0.9), (xx + 0.02, y, z - 0.9),
                                                     (xx + 0.02, y - depth, z - drop), (xx - 0.02, y - depth, z - drop)]],
                                       b.mats["iron"], xf=m))


def clock_turret(b, cx, cy, z0, half, height):
    """A timber-framed clock and bell turret with a tiled spire roof."""
    lo, hi = (cx - half, cy - half, z0), (cx + half, cy + half, z0 + height)
    # The body has no bottom: the posts' bottoms are the only faces there.
    b.block(lo, hi, "plaster", name="TurretBody", skip="-z")
    for sx in (-1, 1):
        for sy in (-1, 1):
            px, py = cx + sx * half, cy + sy * half
            b.beam((px - 0.1, py - 0.1, z0), (px + 0.1, py + 0.1, z0 + height + 0.05), band=DARK_WOOD)
    for zz in (z0 + height - 1.45, z0 + height - 0.12):
        b.beam((cx - half - 0.06, cy - half - 0.06, zz), (cx + half + 0.06, cy - half + 0.04, zz + 0.16))
        b.beam((cx - half - 0.06, cy + half - 0.04, zz), (cx + half + 0.06, cy + half + 0.06, zz + 0.16))
        b.beam((cx - half - 0.06, cy - half - 0.06, zz), (cx - half + 0.04, cy + half + 0.06, zz + 0.16))
        b.beam((cx + half - 0.04, cy - half - 0.06, zz), (cx + half + 0.06, cy + half + 0.06, zz + 0.16))
    zc = z0 + height - 0.75
    for sy, r in ((-1, 0), (1, 180)):
        face = cy + sy * (half + 0.03)
        b.solid("ClockRim", disc_mesh("ClockRim", (cx, face - sy * 0.0, zc), 0.6, r, b.mats["wood"], depth=0.08))
        b.solid("ClockFace", disc_mesh("ClockFace", (cx, face + sy * 0.03, zc), 0.5, r, b.mats["clock"], depth=0.06))
        y = face + sy * 0.07
        b.block((cx - 0.025, y - 0.01, zc), (cx + 0.025, y + 0.01, zc + 0.38), "iron", name="ClockHand")
        b.block((cx, y - 0.01, zc - 0.025), (cx + 0.28, y + 0.01, zc + 0.025), "iron", name="ClockHand")
    # Belfry openings with louvres on the two sides.
    for sx in (-1, 1):
        x = cx + sx * (half + 0.01)
        b.block((x - 0.03, cy - 0.35, z0 + height - 1.25), (x + 0.03, cy + 0.35, z0 + height - 0.25), "iron", name="Belfry")
        for k in range(4):
            zz = z0 + height - 1.15 + k * 0.23
            b.beam((x - 0.05, cy - 0.38, zz), (x + 0.05, cy + 0.38, zz + 0.07), band=DARK_WOOD)
    top = z0 + height + 0.1
    r = (half + 0.35) * math.sqrt(2)
    b.solid("TurretRoof", cone_mesh("TurretRoof", (cx, cy), r, top, top + 2.6, 4, b.mats["tiles"],
                                    rings=3, rot=math.pi / 4, flare=0.18, tile=(1.4, 1.2)))
    b.solid("Spire", cone_mesh("Spire", (cx, cy), 0.09, top + 2.4, top + 3.5, 8, b.mats["iron"], rings=1))
    b.solid("SpireBall", cone_mesh("SpireBall", (cx, cy), 0.13, top + 2.75, top + 2.95, 8, b.mats["paint"], rings=1))


def round_tower(b, cx, cy, r, h, roof_h=3.8, windows=()):
    """A round stone tower with a timber band and a conical tile roof."""
    b.solid("Tower", prism_mesh("Tower", (cx, cy), r, 0.0, h, 16, b.mats["stone"], tile=2.6))
    b.solid("TowerPlinth", prism_mesh("TowerPlinth", (cx, cy), r + 0.1, 0.0, 0.5, 16, b.mats["rock"], tile=1.5))
    b.solid("TowerBand", prism_mesh("TowerBand", (cx, cy), r + 0.08, h - 0.5, h, 16, b.mats["wood"], band=DARK_WOOD))
    b.solid("TowerBand", prism_mesh("TowerBand", (cx, cy), r + 0.06, h * 0.45, h * 0.45 + 0.22, 16, b.mats["wood"], band=LIGHT_WOOD))
    for ang, z in windows:
        a = math.radians(ang)
        d = Vector((math.cos(a), math.sin(a), 0))
        b.put("Window_Thin_Round1", Vector((cx, cy, z)) + d * (r + 0.07), ang + 90)
    b.solid("TowerRoof", cone_mesh("TowerRoof", (cx, cy), r + 0.45, h, h + roof_h, 16, b.mats["tiles"],
                                   rings=4, flare=0.25, tile=(1.6, 1.3)))
    b.solid("Spire", cone_mesh("Spire", (cx, cy), 0.1, h + roof_h - 0.3, h + roof_h + 1.0, 8, b.mats["iron"], rings=1))
    b.collide("tower", (cx - r, cy - r, 0), (cx + r, cy + r, h + roof_h))


def border(b, x0, x1, y, rot=0):
    """The kit's low stone border along a wall's foot."""
    n = int(round((x1 - x0) / 2))
    m = frame(rot, (0, 0, 0))
    for i in range(n):
        at = m @ Vector((x0 + 1 + 2 * i, y, 0))
        b.put("Prop_ExteriorBorder_Straight1", at, rot)


# --------------------------------------------------------------------------
# Buildings

BUILDINGS = {}


def building(fn):
    BUILDINGS[fn.__name__] = fn
    return fn


@building
def townhouse_jettied():
    """Three storeys, each jettied further over the street, front gable."""
    b = Building("townhouse_jettied", {"plaster": "cream", "roof": "red", "timber": "light"})
    w, d = 6, 8
    b.box_storey(w, d, 0, "WDF", sides=("BFBB", "BBWB"), back="BFB")
    b.jetty(w, STOREY, 0.6)
    b.box_storey(w, d, STOREY, "fGf", sides=("PFPP", "PPFP"), back="PFP", jetty=0.6)
    b.jetty(w, 2 * STOREY, 0.6, y=-0.6)
    b.box_storey(w, d, 2 * STOREY, "TWT", sides=("LPPR", "LPFR"), back="LPR", jetty=1.2)
    top = 3 * STOREY + WALL_TOP
    b.gable_roof(w, d + 1.2, top, center=(0, (d - 1.2) / 2))
    b.chimney(-1.6, d - 2.0)
    b.collide("ground", (-3.1, -0.1, 0), (3.1, d + 0.1, STOREY))
    b.collide("upper", (-3.1, -1.3, STOREY), (3.1, d + 0.1, top))
    b.collide("roof", (-3.2, -1.3, top), (3.2, d + 0.1, top + 4.9))
    return b


@building
def townhouse_balcony():
    """Two storeys and an attic, ridge along the street, balcony and dormers."""
    b = Building("townhouse_balcony", {"plaster": "ochre", "roof": "brown", "timber": "mid"})
    w, d = 8, 8
    b.box_storey(w, d, 0, "SBDS", sides=("BSBB", "BBSB"), back="BFFB")
    b.jetty(w, STOREY, 0.5)
    b.box_storey(w, d, STOREY, "TffT", sides=("PFPP", "PPFP"), back="PFFP", jetty=0.5)
    top = 2 * STOREY + WALL_TOP
    # A balcony across the two middle bays, carried on long joists.
    for x in (-1, 1):
        b.put("Floor_WoodDark_Half3", (x, -0.5, STOREY + 0.03), 180)
        b.put("Balcony_Cross_Straight", (x, -0.4, STOREY + 0.02), 0)
    for x in (-1.9, 0.0, 1.9):
        b.put("Roof_Support2", (x, -0.55, STOREY + 0.02), 0, (1.2, 1.25, 1.2))
    b.gable_roof(d + 0.5, w, top, center=(0, (d - 0.5) / 2), along_x=True)
    for x in (-2.0, 2.0):
        b.dormer(x, -0.5)
    b.chimney(2.6, d - 1.6)
    b.collide("ground", (-4.1, -0.1, 0), (4.1, d + 0.1, STOREY))
    b.collide("balcony", (-2.1, -1.6, STOREY), (2.1, -0.5, STOREY + 1.1))
    b.collide("upper", (-4.1, -0.6, STOREY), (4.1, d + 0.1, top))
    b.collide("roof", (-4.2, -1.4, top), (4.2, d + 0.9, top + 5.5))
    return b


@building
def row_townhouse():
    """A narrow three-storey row house for Brownstone Row: stone ground floor,
    blind party walls, a door hood, and a steep front gable."""
    b = Building("row_townhouse", {"plaster": "rose", "roof": "slate", "timber": "dark"})
    w, d = 4, 8
    b.box_storey(w, d, 0, "tD", sides="PPPP", back="tP", stone=True,
                 corner="Corner_Exterior_Wood")
    b.jetty(w, STOREY, 0.4)
    b.box_storey(w, d, STOREY, "WW", sides="PPPP", back="FF", jetty=0.4)
    b.box_storey(w, d, 2 * STOREY, "TT", sides=("LPPR", "LPPR"), back="TT", jetty=0.4)
    top = 3 * STOREY + WALL_TOP
    b.gable_roof(w, d + 0.4, top, center=(0, (d - 0.4) / 2))
    b.chimney(0.9, d - 1.2, piece="Prop_Chimney2", above=1.2)
    b.put("Window_Roof_Wide", (1.0, 0.0, 0.42), 0, (1.0, 0.8, 1.0))
    b.block((0.2, -0.75, 0.0), (1.8, -0.1, 0.18), "rock", name="Step", scale=1.0)
    b.collide("ground", (-2.1, -0.1, 0), (2.1, d + 0.1, STOREY))
    b.collide("step", (0.2, -0.75, 0), (1.8, -0.1, 0.18))
    b.collide("upper", (-2.1, -0.5, STOREY), (2.1, d + 0.1, top))
    b.collide("roof", (-2.8, -0.5, top), (2.8, d + 0.1, top + 3.7))
    return b


@building
def library():
    """A two-storey stone-and-timber hall on a plinth: tall arched windows,
    a gabled entrance bay up a flight of steps, a clock and bell turret on the
    ridge, and a reading-room bay on the east side."""
    b = Building("library", {"plaster": "white", "roof": "slate", "timber": "mid"})
    w, d, base, tall = 12, 10, 0.6, 4.0
    b.block((-w / 2 - 0.25, -0.25, 0), (w / 2 + 0.25, d + 0.25, base), "stone", name="Plinth", scale=2.6)
    b.block((-2.25, -2.25, 0), (2.25, 0.0, base), "stone", name="Plinth", scale=2.6)
    # Ground floor: stone, 4 m high, tall arched windows.
    b.box_storey(w, d, base, ["w", "w", "_", "_", "w", "w"], sides=("PwPwP", "wP__P"),
                 back="PwPPwP", height=tall, stone=True)
    z1 = base + tall
    b.box_storey(w, d, z1, ["f", "T", "_", "_", "T", "f"], sides=("PPfPP", "PPfPP"), back="PPfPPP")
    top = z1 + STOREY + WALL_TOP
    # The entrance bay, 4 m wide and 2 m deep, two storeys under a cross
    # gable, with double doors up a flight of steps.
    b.run([("E", 4)], (-2, -2, base), 0, base, height=tall, stone=True)
    b.run(["T"], (-2, 0, base), -90, base, height=tall, stone=True)
    b.run(["T"], (2, -2, base), 90, base, height=tall, stone=True)
    b.run(["W", "W"], (-2, -2, z1), 0, z1)
    b.run(["P"], (-2, 0, z1), -90, z1)
    b.run(["P"], (2, -2, z1), 90, z1)
    b.corners([(-2, -2), (2, -2)], base, tall)
    b.corners([(-2, -2), (2, -2)], z1)
    b.gable_roof(4, 4.5, top, center=(0, 0.25), ends=(True, False))
    b.put("Stairs_Exterior_Straight", (0, -3.0, 0.0), 0, (1.6, 1.0, base))
    b.gable_roof(d, w, top, center=(0, d / 2), along_x=True, size=8, piece_len=10)
    clock_turret(b, 0, d / 2, top + 4.4, 1.1, 3.3)
    b.collide("turret", (-1.2, d / 2 - 1.2, top + 6.0), (1.2, d / 2 + 1.2, top + 7.7))
    # Reading-room bay on the east wall: lattice windows under a lean-to.
    bx = w / 2
    # The lattice run stops short of the end walls, so its end faces sit
    # inside them.
    b.run(inset([("G", 2), ("G", 2)]), (bx + 1.2, 2.0 + INSET, base), 90, base)
    b.run([("P", 1.2)], (bx, 2.0, base), 0, base)
    b.run([("P", 1.2)], (bx + 1.2, 6.0, base), 180, base)
    b.run(["P", "P"], (bx, 2.0, base + STOREY), 90, base + STOREY, height=1.0, stone=True)
    b.corners([(bx + 1.2, 2.0), (bx + 1.2, 6.0)], base)
    b.block((bx, 2.0, base + STOREY), (bx + 1.25, 6.0, base + STOREY + 0.1), "wood", name="BayCap", scale=2.0)
    for i, piece in enumerate(["Roof_Wooden_2x1_L", "Roof_Wooden_2x1_R"]):
        b.put(piece, (bx, 3.0 + 2 * i, base + STOREY + 0.55), 90, (1, 0.95, 1))
    b.collide("plinth", (-w / 2 - 0.25, -2.25, 0), (w / 2 + 0.25, d + 0.25, base))
    b.collide("steps", (-1.6, -4.0, 0), (1.6, -2.0, base))
    b.collide("entrance", (-2.1, -2.1, base), (2.1, 0, top))
    b.collide("hall", (-w / 2 - 0.1, -0.1, base), (w / 2 + 0.1, d + 0.1, top))
    b.collide("reading_bay", (w / 2, 2.0, base), (w / 2 + 1.3, 6.0, base + STOREY + 0.6))
    b.collide("roof", (-w / 2 - 0.8, -1.2, top), (w / 2 + 0.8, d + 1.2, top + 6.0))
    return b


@building
def tavern():
    """The Lantern Quarter's tavern and music hall: a wide jettied front,
    double doors between lanterns, a hanging sign, and dormers."""
    b = Building("tavern", {"plaster": "ochre", "roof": "green", "timber": "dark"})
    w, d = 12, 8
    b.box_storey(w, d, 0, ["W", "f", ("E", 4), "f", "W"], sides=("BWPB", "BPWB"), back="BPDPFB")
    b.jetty(w, STOREY, 0.6)
    b.box_storey(w, d, STOREY, "tfGGft", sides=("PFPP", "PPFP"), back="PFPPPP", jetty=0.6)
    top = 2 * STOREY + WALL_TOP
    b.gable_roof(d + 0.6, w, top, center=(0, (d - 0.6) / 2), along_x=True, size=8, piece_len=10)
    for x in (-3.0, 3.0):
        b.dormer(x, -0.6)
    b.chimney(-4.2, d - 1.8, piece="Prop_Chimney2", above=1.3)
    b.chimney(4.4, d - 1.8, piece="Prop_Chimney2", above=1.3)
    for x in (-2.75, 2.75):
        lantern(b, x, -0.09, 2.25)
    hanging_sign(b, 4.0, -0.69, 4.1)
    b.collide("ground", (-w / 2 - 0.1, -0.1, 0), (w / 2 + 0.1, d + 0.1, STOREY))
    b.collide("upper", (-w / 2 - 0.1, -0.7, STOREY), (w / 2 + 0.1, d + 0.1, top))
    b.collide("roof", (-w / 2 - 0.8, -1.6, top), (w / 2 + 0.8, d + 1.0, top + 6.0))
    return b


@building
def market_hall():
    """An open timber arcade on stone footings under a jettied upper hall,
    with twin front gables."""
    b = Building("market_hall", {"plaster": "white", "roof": "red", "timber": "mid"})
    w, d = 12, 8
    # The arcade: arches on three sides, a closed back wall.
    b.box_storey(w, d, 0, "AAAAAA", sides="AAAA", back="PBPPBP", corner="Corner_ExteriorWide_Wood")
    for i in range(1, int(w / 2)):
        x = -w / 2 + 2 * i
        b.put("Corner_ExteriorWide_Wood", (x, 0.0, 0))
    for i in range(1, int(d / 2)):
        for x in (-w / 2, w / 2):
            b.put("Corner_ExteriorWide_Wood", (x, 2.0 * i, 0))
    # Stone footings under every post.
    posts = [(-w / 2 + 2 * i, 0.0) for i in range(int(w / 2) + 1)]
    posts += [(x, 2.0 * i) for i in range(1, int(d / 2) + 1) for x in (-w / 2, w / 2)]
    for x, y in posts:
        b.block((x - 0.26, y - 0.26, 0), (x + 0.26, y + 0.26, 0.45), "rock", name="Footing", scale=0.6)
    for i in range(int(w / 2)):
        for j in range(int(d / 2)):
            at = (-w / 2 + 1 + 2 * i, 1 + 2 * j)
            b.put("Floor_UnevenBrick", (*at, 0.02))
    # The ceiling stops inside the back wall, short of its outer face.
    b.beam((-w / 2, 0.0, STOREY - 0.06), (w / 2, d - 0.05, STOREY - 0.04), band=DARK_WOOD)
    b.put("Prop_Crate", (4.2, 5.8, 0.02), 20)
    b.put("Prop_Crate", (3.1, 6.6, 0.02), -10, (0.8, 0.8, 0.8))
    b.jetty(w, STOREY, 0.6)
    b.box_storey(w, d, STOREY, "fTffTf", sides=("PFPP", "PPFP"), back="PPFFPP", jetty=0.6)
    top = 2 * STOREY + WALL_TOP
    for x in (-3, 3):
        b.gable_roof(6, d + 0.6, top, center=(x, (d - 0.6) / 2))
    for o in list(b.col.objects):
        if o.name.startswith("Roof_Front_Brick6") and o.location.x > 0:
            o.location.y -= 0.012 if o.location.y < 1 else -0.012
    for x, y in posts:
        b.collide("post", (x - 0.26, y - 0.26, 0), (x + 0.26, y + 0.26, STOREY))
    b.collide("back_wall", (-w / 2, d - 0.1, 0), (w / 2, d + 0.31, STOREY))
    b.collide("upper", (-w / 2 - 0.1, -0.7, STOREY), (w / 2 + 0.1, d + 0.1, top))
    b.collide("roof", (-w / 2 - 1.1, -1.6, top), (w / 2 + 1.1, d + 0.9, top + 4.9))
    return b


@building
def corner_shop():
    """A corner shop: big shop windows on two faces under striped awnings."""
    b = Building("corner_shop", {"plaster": "rose", "roof": "brown", "timber": "light"})
    w, d = 6, 8
    b.box_storey(w, d, 0, "fDf", sides=("BWPB", "ffPB"), back="BQB")
    b.box_storey(w, d, STOREY, "SGS", sides=("PFPP", "WPFP"), back="PFP")
    top = 2 * STOREY + WALL_TOP
    b.gable_roof(w, d, top, center=(0, d / 2))
    b.chimney(-1.5, d - 1.5)
    awning(b, -3.0, -1.0, -0.12, 2.75)
    awning(b, 1.0, 3.0, -0.12, 2.75)
    # The side shop windows face the cross street (+X).
    awning(b, 0.0, 4.0, -3.12, 2.75, rot=90)
    hanging_sign(b, 3.09, 5.2, 3.9, rot=90, reach=1.2)
    border(b, -3, 3, -0.05)
    b.collide("shop", (-w / 2 - 0.1, -0.1, 0), (w / 2 + 0.1, d + 0.1, top))
    b.collide("roof", (-w / 2 - 1.1, -0.9, top), (w / 2 + 1.1, d + 0.9, top + 4.9))
    return b


@building
def l_house():
    """An L-shaped house: a side-gabled main block with a gabled wing that
    reaches forward to the street."""
    b = Building("l_house", {"plaster": "cream", "roof": "green", "timber": "mid"})
    # Wing: x -4..0, y 0..4. Main block: x -4..4, y 4..10.
    for z, wing, front, left, inner, right, back in (
        (0, "WB", "BD", "BWBFB", "FB", "BFB", "BFFB"),
        (STOREY, "TT", "fW", "PTPFP", "fP", "PFP", "PFFP"),
    ):
        b.run(wing, (-4, 0, z), 0, z)
        # The front stops short of the corner, inside the east wall.
        b.run(inset(front), (INSET, 4, z), 0, z)
        b.run(left, (-4, 10, z), -90, z)
        b.run(inner, (0, 0, z), 90, z)
        b.run(right, (4, 4, z), 90, z)
        b.run(back, (4, 10, z), 180, z)
        b.corners([(-4, 0), (0, 0), (0, 4), (4, 4), (4, 10), (-4, 10)], z)
    top = 2 * STOREY + WALL_TOP
    b.gable_roof(6, 8, top, center=(0, 7), along_x=True)
    b.gable_roof(4, 5.3, top, center=(-2, 2.65), ends=(True, False))
    b.chimney(2.4, 8.4)
    b.put("Window_Roof_Wide", (3.0, 4.0, 0.42), 0, (1.0, 0.8, 1.0))
    b.collide("wing", (-4.1, -0.1, 0), (0.1, 4.0, top))
    b.collide("main", (-4.1, 4.0, 0), (4.1, 10.1, top))
    b.collide("roof", (-4.9, -0.8, top), (4.9, 11.2, top + 4.9))
    return b


@building
def cottage_tower():
    """A one-storey cottage under a steep gable with a round stone tower
    rising at its front corner."""
    b = Building("cottage_tower", {"plaster": "white", "roof": "brown", "timber": "dark"})
    w, d = 6, 6
    b.box_storey(w, d, 0, "PDW", sides=("BFP", "BWB"), back="BFB")
    top = STOREY + WALL_TOP
    b.gable_roof(w, d, top, center=(0, d / 2))
    b.chimney(1.7, d - 1.4)
    round_tower(b, -3.3, 0.6, 1.7, 9.0, windows=((-90, 2.4), (-135, 5.4), (-60, 5.4), (180, 2.4)))
    border(b, -1, 3, -0.05)
    b.collide("cottage", (-3.1, -0.1, 0), (3.1, d + 0.1, top))
    b.collide("roof", (-4.2, -0.9, top), (4.2, d + 0.9, top + 4.9))
    return b


# --------------------------------------------------------------------------
# Cheap generated parts for the second round of buildings


def poly_mesh(name, polys, mat, tile=2.0):
    """Loose polygons, UVs projected on each face's dominant plane."""
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    for poly in polys:
        f = bm.faces.new([bm.verts.new(p) for p in poly])
        f.normal_update()
        n = f.normal
        ax = max(range(3), key=lambda i: abs(n[i]))
        for loop in f.loops:
            co = loop.vert.co
            u, v = [(co.y, co.z), (co.x, co.z), (co.x, co.y)][ax]
            loop[uv].uv = (u / tile, v / tile)
    bm.normal_update()
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(mat)
    return mesh


def frustum_mesh(name, center, r0, r1, z0, z1, sides, mat, tile=2.0, rot=0.0, band=None, cap=True):
    """An upright tapered prism from radius `r0` at `z0` to `r1` at `z1`,
    UV wrapped, with a flat top when `cap`."""
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    cx, cy = center
    ring0, ring1 = [], []
    for i in range(sides + 1):
        a = rot + 2 * math.pi * i / sides
        ring0.append(bm.verts.new((cx + r0 * math.cos(a), cy + r0 * math.sin(a), z0)))
        ring1.append(bm.verts.new((cx + r1 * math.cos(a), cy + r1 * math.sin(a), z1)))
    circ = 2 * math.pi * r0
    for i in range(sides):
        f = bm.faces.new([ring0[i], ring0[i + 1], ring1[i + 1], ring1[i]])
        for loop, (s, h) in zip(f.loops, [(i, z0), (i + 1, z0), (i + 1, z1), (i, z1)]):
            if band is None:
                loop[uv].uv = (circ * s / sides / tile, h / tile)
            else:
                t = (h - z0) / (z1 - z0)
                loop[uv].uv = (circ * s / sides / 2.0, band[0] + (band[1] - band[0]) * t)
    if cap:
        f = bm.faces.new(ring1[:sides])
        for loop in f.loops:
            loop[uv].uv = (loop.vert.co.x / tile, loop.vert.co.y / tile)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    bm.normal_update()
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(mat)
    return mesh


def log_mesh(name, p0, p1, r, mat, sides=6, band=DARK_WOOD):
    """A horizontal log between `p0` and `p1`, along the wood atlas's band."""
    p0, p1 = Vector(p0), Vector(p1)
    axis = p1 - p0
    length = axis.length
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    rings = []
    for x in (0.0, length):
        rings.append([bm.verts.new((x, r * math.cos(2 * math.pi * i / sides), r * math.sin(2 * math.pi * i / sides)))
                      for i in range(sides)])
    for i in range(sides):
        j = (i + 1) % sides
        f = bm.faces.new([rings[0][i], rings[1][i], rings[1][j], rings[0][j]])
        for loop, (u, k) in zip(f.loops, [(0, i), (length, i), (length, i + 1), (0, i + 1)]):
            loop[uv].uv = (u / 2.0, band[0] + (band[1] - band[0]) * k / sides)
    for ring in (rings[0], rings[1][::-1]):
        f = bm.faces.new(ring)
        for loop in f.loops:
            # The end grain: a light corner of the wood atlas.
            loop[uv].uv = (0.3 + 0.3 * loop.vert.co.y, 0.85 + 0.1 * loop.vert.co.z)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    yaw = math.atan2(axis.y, axis.x)
    pitch = math.atan2(axis.z, math.hypot(axis.x, axis.y))
    xf = Matrix.Translation(p0) @ Matrix.Rotation(yaw, 4, "Z") @ Matrix.Rotation(-pitch, 4, "Y")
    bmesh.ops.transform(bm, matrix=xf, verts=bm.verts)
    bm.normal_update()
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(mat)
    return mesh


def slab_roof(b, span, length, z, rise, center=(0.0, 0.0), along_x=False, mat="tiles", over=0.45,
              thick=0.18, gable="plaster", tile=2.0, ridge_mat=None, courses=()):
    """A cheap gabled roof of two textured slabs and two gable triangles.

    The ridge runs along Y through `center` (along X with `along_x`), `rise`
    above the eaves at `z`, and the slabs reach `over` past the walls.
    `courses` lays strips of the ridge's material across each slope at those
    fractions of its run, as on thatch. Returns the ridge's height.
    """
    cx, cy = center
    half = span / 2
    ang = math.atan2(rise, half)
    run = (half + over) / math.cos(ang)
    turn = Matrix.Translation(Vector((cx, cy, 0))) @ Matrix.Rotation(math.radians(-90 if along_x else 0), 4, "Z")
    for flip in (0, 180):
        xf = (turn @ Matrix.Rotation(math.radians(flip), 4, "Z") @ Matrix.Translation(Vector((0, 0, z + rise)))
              @ Matrix.Rotation(ang, 4, "Y"))
        b.solid("RoofSlab", box_mesh("RoofSlab", (0.0, -length / 2 - over, -thick), (run, length / 2 + over, 0.0),
                                     b.mats[mat], tile=tile, xf=xf))
        for t in courses:
            x = run * t
            # Each course runs 2 cm past the slab's ends, so their end faces
            # don't share a plane.
            b.solid("Course", box_mesh("Course", (x - 0.14, -length / 2 - over - 0.02, -0.04),
                                       (x + 0.14, length / 2 + over + 0.02, 0.07),
                                       b.mats[ridge_mat or mat], xf=xf))
    if gable:
        tris = []
        for y in (-length / 2, length / 2):
            tri = [Vector((-half, y, z)), Vector((half, y, z)), Vector((0, y, z + rise - 0.05))]
            tri = [turn @ p for p in tri]
            tris.append(tri if y > 0 else tri[::-1])
        b.solid("Gable", poly_mesh("Gable", tris, b.mats[gable]))
    rid = Matrix.Translation(Vector((0, 0, z + rise)))
    b.solid("Ridge", box_mesh("Ridge", (-0.14, -length / 2 - over - 0.05, -0.1), (0.14, length / 2 + over + 0.05, 0.12),
                              b.mats[ridge_mat or mat], band=DARK_WOOD if (ridge_mat or mat) == "wood" else None,
                              xf=turn @ rid))
    return z + rise


def lean_to(b, x0, x1, y0, y1, z_high, z_low, mat="wood", band=DARK_WOOD, thick=0.12):
    """A single-slope roof falling from `z_high` at x0 to `z_low` at x1."""
    dx = x1 - x0
    ang = math.atan2(z_high - z_low, abs(dx))
    length = math.hypot(dx, z_high - z_low)
    sign = 1 if dx > 0 else -1
    xf = (Matrix.Translation(Vector((x0, 0, z_high))) @ Matrix.Rotation(math.radians(0 if sign > 0 else 180), 4, "Z")
          @ Matrix.Rotation(ang, 4, "Y"))
    if sign < 0:
        y0, y1 = -y1, -y0
    b.solid("LeanTo", box_mesh("LeanTo", (0.0, y0, -thick), (length, y1, 0.0), b.mats[mat],
                               band=band if mat == "wood" else None, xf=xf))


def bell_cote(b, x, y, z, s=0.55):
    """A little open bell-cote with a pyramid cap."""
    for sx in (-1, 1):
        for sy in (-1, 1):
            b.beam((x + sx * s - 0.07, y + sy * s - 0.07, z - 0.4), (x + sx * s + 0.07, y + sy * s + 0.07, z + 1.3),
                   band=DARK_WOOD)
    b.block((x - s - 0.1, y - s - 0.1, z - 0.4), (x + s + 0.1, y + s + 0.1, z - 0.25), "wood", name="CoteFloor")
    b.solid("Bell", cone_mesh("Bell", (x, y), 0.3, z + 0.2, z + 0.85, 8, b.mats["metal"], rings=1, flare=0.06))
    b.solid("CoteRoof", cone_mesh("CoteRoof", (x, y), (s + 0.35) * math.sqrt(2), z + 1.3, z + 2.3, 4, b.mats["tiles"],
                                  rings=1, rot=math.pi / 4, tile=(1.4, 1.2)))
    b.solid("CoteSpire", cone_mesh("CoteSpire", (x, y), 0.05, z + 2.2, z + 2.8, 6, b.mats["iron"], rings=1))


def clock_face(b, cx, cy, z, rot, r=0.75):
    """A clock face on a wall facing `rot` degrees (0 faces -Y)."""
    a = math.radians(rot)
    out = Vector((math.sin(a), -math.cos(a), 0))
    c = Vector((cx, cy, z))
    b.solid("ClockRim", disc_mesh("ClockRim", c + out * 0.04, r, rot, b.mats["wood"], depth=0.08))
    b.solid("ClockFace", disc_mesh("ClockFace", c + out * 0.09, r * 0.84, rot, b.mats["clock"], depth=0.05))
    m = frame(rot, c + out * 0.13)
    b.solid("ClockHand", box_mesh("ClockHand", (-0.03, -0.01, 0.0), (0.03, 0.01, r * 0.62), b.mats["iron"], xf=m))
    b.solid("ClockHand", box_mesh("ClockHand", (0.0, -0.01, -0.03), (r * 0.45, 0.01, 0.03), b.mats["iron"], xf=m))


def banner(b, x, y, z, rot=0, w=0.8, h=1.8, cloth="cloth4"):
    """A hanging cloth banner with a gold roundel, from a rod on a wall."""
    m = frame(rot, (x, y, z))
    b.solid("BannerRod", box_mesh("BannerRod", (-w / 2 - 0.08, -0.16, -0.03), (w / 2 + 0.08, -0.1, 0.03),
                                  b.mats["iron"], xf=m))
    quad = [(-w / 2, -0.12, -0.05), (w / 2, -0.12, -0.05), (w / 2, -0.12, -h), (0, -0.12, -h - 0.3),
            (-w / 2, -0.12, -h)]
    b.solid("Banner", poly_mesh("Banner", [[m @ Vector(p) for p in quad]], b.mats[cloth]))
    b.solid("BannerRoundel", disc_mesh("BannerRoundel", m @ Vector((0, -0.14, -h * 0.45)), w * 0.3, rot,
                                       b.mats["paint"], depth=0.02))


def anvil(b, x, y):
    b.block((x - 0.3, y - 0.3, 0), (x + 0.3, y + 0.3, 0.5), "wood", name="AnvilStump", scale=1.0)
    b.block((x - 0.12, y - 0.3, 0.5), (x + 0.12, y + 0.3, 0.72), "iron", name="AnvilWaist")
    b.block((x - 0.2, y - 0.45, 0.72), (x + 0.2, y + 0.38, 0.9), "iron", name="AnvilFace")
    horn = cone_mesh("AnvilHorn", (0, 0), 0.12, 0, 0.4, 4, b.mats["iron"], rings=1)
    horn.transform(Matrix.Translation(Vector((x, y - 0.45, 0.82))) @ Matrix.Rotation(math.radians(90), 4, "X"))
    b.solid("AnvilHorn", horn)


# --------------------------------------------------------------------------
# The second round: buildings the map names


@building
def music_hall():
    """The Lantern Quarter's Music Hall: an octagonal hall of tall arched
    windows on a stone plinth, under a tiled cone with a lit lantern cupola."""
    b = Building("music_hall", {"plaster": "white", "roof": "teal", "timber": "dark"})
    side, base, tall = 4.6, 0.5, 4.5
    R = side / (2 * math.sin(math.pi / 8))
    a = R * math.cos(math.pi / 8)
    c = Vector((0.0, a, 0.0))
    rot0 = math.radians(-67.5)
    b.solid("Plinth", frustum_mesh("Plinth", c.xy, R + 0.45, R + 0.4, 0.0, base, 8, b.mats["rock"], tile=1.5, rot=rot0))
    b.block((-2.0, -1.2, 0.0), (2.0, 0.0, 0.25), "rock", name="Step", scale=1.0)
    pts = []
    for k in range(8):
        theta = math.radians(-90 + 45 * k)
        v = c + Vector((R * math.cos(theta - math.pi / 8), R * math.sin(theta - math.pi / 8), 0))
        pts.append((v.x, v.y))
        if k == 0:
            tokens = [("D", side)]
        elif k == 4:
            tokens = [("P", side / 2), ("Q", side / 2)]
        else:
            tokens = [("w", side / 2), ("w", side / 2)]
        b.run(tokens, (v.x, v.y, base), math.degrees(theta) + 90, base, height=tall)
    b.corners(pts, base, tall)
    top = base + tall * (STOREY + WALL_TOP) / STOREY
    b.solid("Frieze", frustum_mesh("Frieze", c.xy, R + 0.12, R + 0.12, top - 0.35, top + 0.05, 8, b.mats["wood"],
                                   rot=rot0, band=DARK_WOOD, cap=False))
    rh = 5.2
    b.solid("Roof", cone_mesh("Roof", c.xy, R + 0.9, top, top + rh, 8, b.mats["tiles"], rings=3, rot=rot0,
                              flare=0.3, tile=(1.8, 1.6)))
    # The lantern cupola: a ring of lit glass under a little cone.
    z = top + rh * (1 - 1.5 / (R + 0.9)) - 0.2
    b.solid("CupolaBase", frustum_mesh("CupolaBase", c.xy, 1.55, 1.5, z, z + 0.3, 8, b.mats["wood"], rot=rot0,
                                       band=DARK_WOOD))
    b.solid("CupolaGlass", frustum_mesh("CupolaGlass", c.xy, 1.3, 1.3, z + 0.3, z + 1.4, 8, b.mats["lamp"], rot=rot0))
    for k in range(8):
        a8 = rot0 + math.pi * k / 4
        p = c + Vector((1.38 * math.cos(a8), 1.38 * math.sin(a8), 0))
        b.beam((p.x - 0.07, p.y - 0.07, z + 0.3), (p.x + 0.07, p.y + 0.07, z + 1.45), band=DARK_WOOD)
    b.solid("CupolaRoof", cone_mesh("CupolaRoof", c.xy, 1.9, z + 1.4, z + 3.0, 8, b.mats["tiles"], rings=1, rot=rot0,
                                    flare=0.12))
    b.solid("Finial", cone_mesh("Finial", c.xy, 0.08, z + 2.8, z + 3.9, 6, b.mats["iron"], rings=1))
    b.solid("FinialBall", cone_mesh("FinialBall", c.xy, 0.16, z + 3.2, z + 3.45, 8, b.mats["paint"], rings=1))
    for x in (-1.75, 1.75):
        lantern(b, x, -0.09, base + 2.5)
    b.collide("hall_x", (-a, a - side / 2, 0), (a, a + side / 2, top))
    b.collide("hall_y", (-side / 2, 0.0, 0), (side / 2, 2 * a, top))
    b.collide("diag", (-a + 1.2, 1.2, 0), (a - 1.2, 2 * a - 1.2, top))
    b.collide("step", (-2.0, -1.2, 0), (2.0, 0.0, 0.25))
    b.front = (0.0, -1.9)
    return b


@building
def meeting_hall():
    """The meeting hall: a tall stone hall under a broad gable, a columned
    porch at its door, and a bell-cote on the ridge."""
    b = Building("meeting_hall", {"plaster": "sage", "roof": "slate", "timber": "dark"})
    w, d, tall = 12, 10, 4.0
    b.box_storey(w, d, 0, ["t", "w", ("E", 4), "w", "t"], sides=("BwBwB", "BwBwB"), back="BtwwtB",
                 height=tall, stone=True)
    top = tall * (STOREY + WALL_TOP) / STOREY
    b.gable_roof(w, d + 0.6, top, center=(0, d / 2), size=8, piece_len=10)
    ridge = b.roof_z(0, d / 2)
    # The porch: four posts on a stone floor under a small cross gable.
    b.block((-2.6, -2.6, 0), (2.6, 0, 0.2), "rock", name="PorchFloor", scale=1.0)
    for x in (-2.3, -0.9, 0.9, 2.3):
        b.beam((x - 0.13, -2.45, 0.2), (x + 0.13, -2.19, top - 0.1), band=LIGHT_WOOD)
    b.beam((-2.5, -2.55, top - 0.25), (2.5, -2.1, top), band=DARK_WOOD)
    b.gable_roof(5.4, 3.0, top, center=(0, -1.25), ends=(True, False), size=4)
    bell_cote(b, 0, 1.6, b.roof_z(0, 1.6) + 0.3)
    b.chimney(-4.0, d - 1.5, piece="Prop_Chimney2", above=1.2)
    for x in (-3.2, 3.2):
        lantern(b, x, -0.09, 2.6)
    b.collide("hall", (-w / 2 - 0.1, -0.1, 0), (w / 2 + 0.1, d + 0.1, top))
    b.collide("porch", (-2.45, -2.5, 0), (-2.15, -2.15, top))
    b.collide("porch", (2.15, -2.5, 0), (2.45, -2.15, top))
    b.roofs.append(((0.0, d / 2), False, (w / 2 + 0.8, d / 2 + 0.3), top, ridge))
    b.front = (0.0, -3.4)
    return b


@building
def boathouse():
    """A timber boathouse on a stone footing, its wide arch facing the
    water, with a side door and a hayloft hatch in the gable."""
    b = Building("boathouse", {"plaster": "white", "roof": "brown", "timber": "dark"})
    w, d, base = 6, 8, 0.35
    b.block((-w / 2 - 0.2, -0.2, 0), (w / 2 + 0.2, d + 0.2, base), "stone", name="Footing", scale=2.6)
    b.box_storey(w, d, base, [("A", 6)], sides=("PPQP", "PPPP"), back="PGP", height=3.6)
    top = base + 3.6 * (STOREY + WALL_TOP) / STOREY
    b.block((-w / 2 + 0.2, 0.2, base), (w / 2 - 0.2, d - 0.2, base + 0.04), "wood", name="Floor")
    # A steep, cheap roof of tiled slabs over board gables: the pond's side
    # of the town sees it from every bench.
    ridge = slab_roof(b, w, d, top + 0.15, 3.4, center=(0, d / 2), mat="tiles", over=0.55, thick=0.2,
                      gable="boards", ridge_mat="wood")
    b.beam((-w / 2 - 0.1, -0.2, top - 0.3), (w / 2 + 0.1, 0.05, top), band=DARK_WOOD)
    b.block((-0.6, -0.12, top + 0.4), (0.6, 0.0, top + 1.6), "boards", name="Hatch", scale=1.0)
    b.beam((-0.7, -0.16, top + 0.3), (0.7, -0.04, top + 0.4), band=LIGHT_WOOD)
    b.beam((-0.7, -0.16, top + 1.6), (0.7, -0.04, top + 1.7), band=LIGHT_WOOD)
    lantern(b, -w / 2, 3.0 + 1.0, base + 2.3, rot=-90)
    b.collide("boathouse", (-w / 2 - 0.2, -0.2, 0), (w / 2 + 0.2, d + 0.2, top))
    b.roofs.append(((0.0, d / 2), False, (w / 2 + 0.55, d / 2 + 0.55), top + 0.15 - 0.55 * 3.4 / 3, ridge))
    b.front = (-w / 2 - 1.0, 3.0)
    return b


@building
def boardwalk_cafe():
    """A Boardwalk Café: a one-storey timber café with wide windows under a
    green-striped awning, on a plank deck with rails."""
    b = Building("boardwalk_cafe", {"plaster": "butter", "roof": "red", "timber": "mid"})
    w, d, base = 8, 6, 0.15
    b.box_storey(w, d, base, ["F", "F", "Q", "F"], sides=("GFG", "GFG"), back="BPPB")
    top = base + STOREY + WALL_TOP
    b.gable_roof(d + 0.6, w, top, center=(0, d / 2), along_x=True, size=6, piece_len=8)
    ridge = b.roof_z(0, d / 2)
    awning(b, -w / 2 + 0.1, w / 2 - 0.1, -0.12, top - 0.35, depth=1.6, drop=0.5)
    for o in b.col.objects:
        if o.name.startswith("Awning.") and o.data.materials and o.data.materials[0] == b.mats["cloth"]:
            o.data.materials[0] = b.mats["cloth3"]
    # The deck: planks over joists, rails at its sides, open at the front.
    b.block((-w / 2 - 0.6, -2.8, 0), (w / 2 + 0.6, 0, base), "wood", name="Deck", scale=2.0)
    for sx in (-1, 1):
        x = sx * (w / 2 + 0.5)
        for y in (-2.7, -1.4, -0.1):
            b.beam((x - 0.06, y - 0.06, base), (x + 0.06, y + 0.06, base + 1.0), band=DARK_WOOD)
        b.beam((x - 0.05, -2.75, base + 0.92), (x + 0.05, 0, base + 1.02), band=LIGHT_WOOD)
    for sx in (-1, 1):
        for x in (sx * (w / 2 + 0.5), sx * 2.2):
            b.beam((x - 0.06, -2.76, base), (x + 0.06, -2.64, base + 1.0), band=DARK_WOOD)
        x0, x1 = sorted((sx * (w / 2 + 0.5), sx * 2.2))
        b.beam((x0, -2.75, base + 0.92), (x1, -2.65, base + 1.02), band=LIGHT_WOOD)
    hanging_sign(b, -w / 2 - 0.02, 1.0, 3.0, rot=-90, reach=1.0)
    b.chimney(2.5, d - 1.2, piece="Prop_Chimney2", above=1.0)
    b.collide("cafe", (-w / 2 - 0.1, -0.1, 0), (w / 2 + 0.1, d + 0.1, top))
    b.roofs.append(((0.0, d / 2), True, (d / 2 + 0.7, w / 2 + 0.3), top, ridge))
    b.front = (1.0, -1.6)
    return b


@building
def bakery():
    """The bakery: two storeys of warm plaster, a round brick bread oven
    built onto its side with a tall chimney, an awning, and a hanging sign."""
    b = Building("bakery", {"plaster": "butter", "roof": "red", "timber": "mid"})
    w, d = 6, 8
    b.box_storey(w, d, 0, "SDf", sides=("BSBB", "BPPB"), back="BFB")
    b.jetty(w, STOREY, 0.4)
    b.box_storey(w, d, STOREY, "TGT", sides=("PFPP", "PPFP"), back="PFP", jetty=0.4)
    top = 2 * STOREY + WALL_TOP
    b.gable_roof(w, d + 0.4, top, center=(0, (d - 0.4) / 2))
    ridge = b.roof_z(0, d / 2)
    # The oven: a brick drum on the east wall under a dome, and its stack.
    ox, oy = w / 2, 4.6
    b.solid("Oven", frustum_mesh("Oven", (ox, oy), 1.5, 1.5, 0.0, 1.6, 12, b.mats["brick"], tile=1.2,
                                 rot=math.pi / 12, cap=False))
    b.solid("OvenDome", cone_mesh("OvenDome", (ox, oy), 1.55, 1.6, 2.5, 12, b.mats["brick"], rings=2, flare=0.05,
                                  tile=(1.2, 1.2), rot=math.pi / 12))
    b.block((ox + 1.0, oy - 0.45, 0.4), (ox + 1.55, oy + 0.45, 1.2), "iron", name="OvenDoor", scale=1.0)
    b.block((ox + 1.02, oy - 0.3, 0.5), (ox + 1.57, oy + 0.3, 1.0), "coals", name="OvenGlow", scale=1.0)
    # The stack's inner face sits inside the wall, clear of its inner face.
    b.block((ox - 0.15, oy + 0.6, 0.0), (ox + 0.8, oy + 1.6, ridge + 1.6), "brick", name="Stack", scale=1.2)
    b.block((ox - 0.3, oy + 0.5, ridge + 1.6), (ox + 0.9, oy + 1.7, ridge + 1.8), "rock", name="StackCap",
            scale=1.0)
    awning(b, -3.0, -1.0, -0.12, 2.75)
    awning(b, 1.0, 3.0, -0.12, 2.75)
    hanging_sign(b, -2.9, -0.5, 4.1, reach=1.3)
    border(b, -3, 3, -0.05)
    b.collide("shop", (-w / 2 - 0.1, -0.1, 0), (w / 2 + 0.1, d + 0.1, STOREY))
    b.collide("oven", (ox, oy - 1.55, 0), (ox + 1.55, oy + 1.6, 2.5))
    b.collide("upper", (-w / 2 - 0.1, -0.5, STOREY), (w / 2 + 0.1, d + 0.1, top))
    b.roofs.append(((0.0, (d - 0.4) / 2), False, (w / 2 + 0.6, d / 2 + 0.4), top, ridge))
    b.front = (-1.0, -0.9)
    return b


@building
def smithy():
    """The Foundry's forge: a low stone smithy under a slate roof with an
    open lean-to forge on its side, a glowing hearth, a chimney, and an
    anvil."""
    b = Building("smithy", {"plaster": "white", "roof": "charcoal", "timber": "dark"})
    w, d = 8, 8
    b.box_storey(w, d, 0, ["B", "Q", "S", "B"], sides=("BSBB", "BBBB"), back="BBSB", stone=True)
    top = STOREY + WALL_TOP
    b.gable_roof(w, d, top, center=(0, d / 2), size=8, piece_len=8)
    ridge = b.roof_z(0, d / 2)
    # The open forge on the east side.
    fx = w / 2
    for y in (0.4, 4.0, 7.6):
        b.beam((fx + 3.2, y - 0.12, 0), (fx + 3.44, y + 0.12, 2.5), band=DARK_WOOD)
    b.beam((fx + 3.15, 0.2, 2.35), (fx + 3.5, 7.8, 2.55), band=DARK_WOOD)
    # The lean-to's ends stop 2 cm inside the front and back walls' faces.
    lean_to(b, fx, fx + 3.8, 0.02, 7.98, 3.1, 2.4)
    b.block((fx + 0.1, 2.5, 0), (fx + 1.6, 5.5, 0.85), "brick", name="Hearth", scale=1.2)
    b.block((fx + 0.3, 2.8, 0.85), (fx + 1.4, 5.2, 0.92), "coals", name="Coals", scale=1.0)
    b.solid("Hood", cone_mesh("Hood", (fx + 0.85, 4.0), 1.2, 1.9, 2.8, 4, b.mats["brick"], rings=1,
                              rot=math.pi / 4, tile=(1.2, 1.2)))
    b.block((fx + 0.35, 3.5, 2.6), (fx + 1.35, 4.5, ridge + 1.3), "brick", name="Chimney", scale=1.2)
    b.block((fx + 0.25, 3.4, ridge + 1.3), (fx + 1.45, 4.6, ridge + 1.5), "rock", name="ChimneyCap", scale=1.0)
    anvil(b, fx + 2.4, 2.0)
    b.block((fx + 2.0, 6.2, 0), (fx + 3.0, 7.4, 0.6), "wood", name="Trough", scale=1.0)
    b.block((fx + 2.08, 6.28, 0.5), (fx + 2.92, 7.32, 0.56), "glass", name="TroughWater", scale=1.0)
    b.collide("smithy", (-w / 2 - 0.1, -0.1, 0), (w / 2 + 0.1, d + 0.1, top))
    b.collide("forge", (fx, 0.2, 0), (fx + 3.5, 7.8, 2.4))
    b.roofs.append(((0.0, d / 2), False, (w / 2 + 0.7, d / 2 + 0.3), top, ridge))
    b.front = (-1.0, -0.9)
    return b


@building
def windmill():
    """A tower windmill for the long meadow: a tapering plaster tower on a
    stone foot, a tiled cap, and four lattice sails with canvas."""
    b = Building("windmill", {"plaster": "white", "roof": "brown", "timber": "mid"})
    r0, r1, h = 3.2, 2.2, 9.0
    a0 = r0 * math.cos(math.pi / 8)
    cy = a0
    rot0 = math.radians(-67.5)
    b.solid("Foot", frustum_mesh("Foot", (0, cy), r0 + 0.25, r0 + 0.05, 0.0, 0.9, 8, b.mats["stone"], tile=2.6,
                                 rot=rot0, cap=False))
    b.solid("Tower", frustum_mesh("Tower", (0, cy), r0, r1, 0.0, h, 8, b.mats["plaster"], tile=3.0, rot=rot0))
    for z, k in ((3.6, 0.0), (6.6, 0.0)):
        rr = r0 + (r1 - r0) * z / h
        b.solid("Band", frustum_mesh("Band", (0, cy), rr + 0.07, rr + 0.06, z, z + 0.22, 8, b.mats["wood"],
                                     rot=rot0, band=DARK_WOOD, cap=False))
    # The door and windows stand on the faces, leaning with them.
    b.put("DoorFrame_Round_WoodDark", (0, 0.08, 0), 0)
    b.put("Door_1_Round", (-0.55, 0.1, 0), 0)
    for k, z in ((2, 4.2), (6, 4.2), (1, 6.8), (7, 6.8), (4, 5.5)):
        theta = math.radians(-90 + 45 * k)
        rz = (r0 + (r1 - r0) * (z + 1.3) / h) * math.cos(math.pi / 8)
        p = Vector((rz * math.cos(theta), cy + rz * math.sin(theta), z))
        b.put("Window_Thin_Round1", p + Vector((math.cos(theta), math.sin(theta), 0)) * 0.12,
              math.degrees(theta) + 90)
    # The cap: a short drum and a tiled cone, turned to the wind.
    b.solid("CapRing", frustum_mesh("CapRing", (0, cy), r1 + 0.35, r1 + 0.3, h, h + 0.35, 8, b.mats["wood"],
                                    rot=rot0, band=DARK_WOOD))
    b.solid("Cap", cone_mesh("Cap", (0, cy), r1 + 0.55, h + 0.3, h + 3.0, 8, b.mats["tiles"], rings=3, rot=rot0,
                             flare=0.15, tile=(1.6, 1.4)))
    b.solid("CapFinial", cone_mesh("CapFinial", (0, cy), 0.1, h + 2.8, h + 3.5, 6, b.mats["iron"], rings=1))
    hub = Vector((0, cy - r1 - 0.9, h + 0.9))
    b.block((hub.x - 0.25, hub.y, hub.z - 0.25), (hub.x + 0.25, cy - r1 + 0.3, hub.z + 0.25), "wood", name="Shaft",
            scale=1.0)
    b.block((hub.x - 0.35, hub.y - 0.25, hub.z - 0.35), (hub.x + 0.35, hub.y + 0.1, hub.z + 0.35), "iron",
            name="Hub", scale=1.0)
    length, width = 7.2, 1.7
    for k in range(4):
        m = Matrix.Translation(hub) @ Matrix.Rotation(math.radians(45 + 90 * k), 4, "Y")
        b.solid("Stock", box_mesh("Stock", (-0.1, -0.22, 0.2), (0.1, -0.08, length), b.mats["wood"],
                                  band=LIGHT_WOOD, xf=m))
        for x in (0.15, width):
            b.solid("SailRail", box_mesh("SailRail", (x - 0.05, -0.2, 1.2), (x + 0.05, -0.1, length - 0.1),
                                         b.mats["wood"], band=LIGHT_WOOD, xf=m))
        for i in range(7):
            z = 1.2 + (length - 1.3) * i / 6
            b.solid("SailBar", box_mesh("SailBar", (0.1, -0.2, z - 0.04), (width + 0.05, -0.1, z + 0.04),
                                        b.mats["wood"], band=LIGHT_WOOD, xf=m))
        b.solid("Canvas", quad_mesh("Canvas", [[(0.2, -0.12, 1.25), (width - 0.05, -0.12, 1.25),
                                                (width - 0.05, -0.12, length - 0.15), (0.2, -0.12, length - 0.15)]],
                                    b.mats["cloth2"], xf=m))
    b.collide("tower", (-r0 - 0.25, cy - a0 - 0.1, 0), (r0 + 0.25, cy + a0 + 0.1, h))
    b.front = (0.0, -1.0)
    return b


@building
def greenhouse():
    """The Community Gardens' glasshouse: a brick base, white glazing bars,
    pale glass, and benches of seedlings inside."""
    b = Building("greenhouse", {"plaster": "white", "roof": "red", "timber": "light"})
    w, d, base, eave, ridge = 4.4, 8.0, 0.6, 2.4, 3.6
    hw = w / 2
    for lo, hi in (((-hw, 0, 0), (hw, 0.22, base)), ((-hw, d - 0.22, 0), (hw, d, base)),
                   ((-hw, 0, 0), (-hw + 0.22, d, base)), ((hw - 0.22, 0, 0), (hw, d, base))):
        b.block(lo, hi, "brick", name="Base", scale=1.2)
    glass = []
    for x in (-hw + 0.11, hw - 0.11):
        glass.append([(x, 0.1, base), (x, d - 0.1, base), (x, d - 0.1, eave), (x, 0.1, eave)])
    for y in (0.11, d - 0.11):
        glass.append([(-hw + 0.1, y, base), (hw - 0.1, y, base), (hw - 0.1, y, eave), (-hw + 0.1, y, eave)])
        glass.append([(-hw + 0.1, y, eave), (hw - 0.1, y, eave), (0, y, ridge)])
    for sx in (-1, 1):
        glass.append([(sx * hw, 0.0, eave + 0.02), (sx * hw, d, eave + 0.02), (0, d, ridge + 0.02),
                      (0, 0.0, ridge + 0.02)])
    b.solid("Glass", poly_mesh("Glass", glass, b.mats["glass_clear"]))
    white = "white_paint"
    for i in range(9):
        y = d * i / 8
        for sx in (-1, 1):
            b.block((sx * hw - 0.05, y - 0.05, base), (sx * hw + 0.05, y + 0.05, eave), white, name="Mullion")
            m = (Matrix.Translation(Vector((sx * hw, y, eave))) @ Matrix.Rotation(math.radians(0 if sx < 0 else 180), 4, "Z")
                 @ Matrix.Rotation(-math.atan2(ridge - eave, hw), 4, "Y"))
            b.solid("Rafter", box_mesh("Rafter", (0.0, -0.05, 0.0), (math.hypot(hw, ridge - eave), 0.05, 0.08),
                                       b.mats[white], xf=m))
    for x in (-1.1, 0.0, 1.1):
        for y in (0.11, d - 0.11):
            if not (x == 0.0 and y < 1):
                b.block((x - 0.04, y - 0.05, base), (x + 0.04, y + 0.05, eave), white, name="Mullion")
    b.block((-hw - 0.06, -0.06, eave - 0.06), (hw + 0.06, d + 0.06, eave + 0.04), white, name="Plate")
    b.block((-0.08, -0.1, ridge - 0.05), (0.08, d + 0.1, ridge + 0.12), white, name="RidgeBar")
    for x in (-0.55, 0.47):
        b.block((x, -0.08, 0), (x + 0.08, 0.08, 2.15), white, name="DoorJamb")
    b.block((-0.55, -0.08, 2.07), (0.55, 0.08, 2.15), white, name="DoorHead")
    b.block((-0.47, -0.04, 1.0), (0.47, 0.04, 1.06), white, name="DoorRail")
    for sx in (-1, 1):
        b.block((sx * 1.3 - 0.5, 0.6, 0.0), (sx * 1.3 + 0.5, d - 0.6, 0.8), "wood", name="Bench", scale=1.0)
        for i in range(7):
            y = 1.1 + i * 0.95
            b.solid("Seedling", cone_mesh("Seedling", (sx * 1.3, y), 0.32, 0.8, 1.45, 5, b.mats["leaf"], rings=1))
    b.collide("greenhouse", (-hw - 0.05, -0.1, 0), (hw + 0.05, d + 0.05, eave))
    b.roofs.append(((0.0, d / 2), False, (hw, d / 2), eave, ridge))
    b.front = (0.0, -0.9)
    return b


@building
def clock_tower():
    """A plaza landmark: a stone clock tower with an open belfry and a tall
    slate spire, and a small chapel-like hall behind it."""
    b = Building("clock_tower", {"plaster": "white", "roof": "slate", "timber": "dark"})
    s, body = 2.2, 12.0
    cz = s
    b.block((-s - 0.2, -0.2, 0), (s + 0.2, 2 * s + 0.2, 0.7), "rock", name="TowerPlinth", scale=1.5)
    b.block((-s, 0, 0.7), (s, 2 * s, body), "stone", name="Tower", scale=2.6)
    for z in (4.6, 8.6, body - 0.2):
        b.block((-s - 0.1, -0.1, z), (s + 0.1, 2 * s + 0.1, z + 0.3), "rock", name="Course", scale=1.0)
    b.put("DoorFrame_Round_WoodDark", (0, -0.02, 0.7), 0)
    b.door("Door_1_Round", Vector((0, -0.04, 0.7)), 0, 1.0, 1.0)
    b.block((-1.0, -1.0, 0), (1.0, 0, 0.35), "rock", name="Step", scale=1.0)
    for z in (5.4,):
        b.put("Window_Thin_Round1", (0, -0.12, z), 0)
        b.put("Window_Thin_Round1", (-s - 0.12, cz, z), -90)
        b.put("Window_Thin_Round1", (s + 0.12, cz, z), 90)
    for rot, (x, y) in ((0, (0, 0)), (90, (s, cz)), (180, (0, 2 * s)), (270, (-s, cz))):
        clock_face(b, x, y, 10.2, rot, r=0.95)
    # The belfry: four piers, a bell, louvres, and a cornice.
    z0 = body + 0.1
    for sx in (-1, 1):
        for sy in (-1, 1):
            px, py = sx * (s - 0.35), cz + sy * (s - 0.35)
            b.block((px - 0.35, py - 0.35, z0), (px + 0.35, py + 0.35, z0 + 2.6), "stone", name="Pier", scale=2.6)
    # The floor rests on the cornice course, so it has no bottom face to
    # share the piers' plane.
    b.block((-s + 0.4, cz - s + 0.4, z0), (s - 0.4, cz + s - 0.4, z0 + 0.15), "wood", name="BelfryFloor",
            skip="-z")
    b.solid("Bell", cone_mesh("Bell", (0, cz), 0.65, z0 + 0.7, z0 + 1.9, 10, b.mats["metal"], rings=1, flare=0.1))
    for k in range(4):
        zz = z0 + 0.9 + k * 0.4
        b.beam((-s + 0.7, -0.05, zz), (s - 0.7, 0.05, zz + 0.1), band=DARK_WOOD)
        b.beam((-s + 0.7, 2 * s - 0.05, zz), (s - 0.7, 2 * s + 0.05, zz + 0.1), band=DARK_WOOD)
    b.block((-s - 0.25, -0.25, z0 + 2.6), (s + 0.25, 2 * s + 0.25, z0 + 3.0), "rock", name="Cornice", scale=1.0)
    top = z0 + 3.0
    b.solid("Spire", cone_mesh("Spire", (0, cz), (s + 0.3) * math.sqrt(2), top, top + 7.0, 4, b.mats["tiles"],
                               rings=3, rot=math.pi / 4, flare=0.2, tile=(1.6, 1.4)))
    b.solid("SpireRod", cone_mesh("SpireRod", (0, cz), 0.07, top + 6.7, top + 8.2, 6, b.mats["iron"], rings=1))
    b.solid("SpireBall", cone_mesh("SpireBall", (0, cz), 0.2, top + 7.0, top + 7.3, 8, b.mats["paint"], rings=1))
    for sx in (-1, 1):
        b.solid("Pinnacle", cone_mesh("Pinnacle", (sx * s, 0), 0.25, top, top + 1.4, 4, b.mats["rock"], rings=1,
                                      rot=math.pi / 4))
        b.solid("Pinnacle", cone_mesh("Pinnacle", (sx * s, 2 * s), 0.25, top, top + 1.4, 4, b.mats["rock"], rings=1,
                                      rot=math.pi / 4))
    # The hall behind: stone walls, tall windows, a steep tiled roof.
    # The hall stands 2 cm behind the tower, so its front wall's faces
    # don't share the planes of the tower's and the plinth's backs.
    hw, y0, y1, tall = 3.0, 2 * s + 0.02, 2 * s + 8.02, 4.0
    b.run("tTtT", (-hw, y1, 0), -90, 0, height=tall, stone=True)
    b.run("TtTt", (hw, y0, 0), 90, 0, height=tall, stone=True)
    b.run("BTB", (hw, y1, 0), 180, 0, height=tall, stone=True)
    b.run("BPB", (-hw, y0, 0), 0, 0, height=tall, stone=True)
    b.corners([(-hw, y0), (hw, y0), (-hw, y1), (hw, y1)], 0, tall)
    htop = tall * (STOREY + WALL_TOP) / STOREY
    b.gable_roof(2 * hw, y1 - y0 + 0.6, htop, center=(0, (y0 + y1) / 2 + 0.3), size=6, piece_len=8,
                 ends=(False, True))
    hall_ridge = b.roof_z(0, (y0 + y1) / 2 + 1.0)
    b.collide("tower", (-s - 0.2, -0.2, 0), (s + 0.2, 2 * s + 0.2, top))
    b.collide("hall", (-hw - 0.1, y0, 0), (hw + 0.1, y1 + 0.1, htop))
    b.collide("step", (-1.0, -1.0, 0), (1.0, 0, 0.35))
    b.roofs.append(((0.0, (y0 + y1) / 2 + 0.3), False, (hw + 0.6, (y1 - y0) / 2 + 0.3), htop, hall_ridge))
    b.front = (0.0, -1.6)
    return b


@building
def guild_hall():
    """A guild hall: a stone ground floor with arched windows and double
    doors, a jettied upper floor, guild banners, dormers, and a round
    corner turret."""
    b = Building("guild_hall", {"plaster": "lilac", "roof": "plum", "timber": "dark"})
    w, d = 12, 10
    b.box_storey(w, d, 0, ["W", "W", ("E", 4), "W", "W"], sides=("BWBWB", "BWBWB"), back="BFBBFB", stone=True)
    b.jetty(w, STOREY, 0.6)
    b.box_storey(w, d, STOREY, "SfGGfS", sides=("PFPFP", "PFPFP"), back="PFPPFP", jetty=0.6)
    top = 2 * STOREY + WALL_TOP
    b.gable_roof(d + 0.6, w, top, center=(0, (d - 0.6) / 2), along_x=True, size=8, piece_len=12)
    ridge = b.roof_z(0, d / 2)
    for x in (-3.0, 3.0):
        b.dormer(x, -0.6)
    b.chimney(-4.5, d - 2.0, piece="Prop_Chimney2", above=1.4)
    b.chimney(4.5, d - 2.0, piece="Prop_Chimney2", above=1.4)
    for x in (-1.5, 1.5):
        banner(b, x, -0.69, 2 * STOREY - 0.15, w=0.8, h=1.7, cloth="cloth4")
    for x in (-2.75, 2.75):
        lantern(b, x, -0.09, 2.25)
    round_tower(b, w / 2 + 0.3, -0.3, 1.3, 8.6, roof_h=3.4, windows=((-60, 2.4), (-30, 5.4), (30, 5.4)))
    b.collide("ground", (-w / 2 - 0.1, -0.1, 0), (w / 2 + 0.1, d + 0.1, STOREY))
    b.collide("upper", (-w / 2 - 0.1, -0.7, STOREY), (w / 2 + 0.1, d + 0.1, top))
    b.roofs.append(((0.0, (d - 0.6) / 2), True, (d / 2 + 1.0, w / 2 + 0.5), top, ridge))
    b.front = (0.0, -0.9)
    return b


@building
def lookout():
    """A timber lookout tower on four legs, with a ladder, a railed platform
    seven meters up, and a pyramid roof."""
    b = Building("lookout", {"plaster": "white", "roof": "green", "timber": "mid"})
    h, s, cy = 7.0, 1.3, 1.5
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = sx * s, cy + sy * s
            b.beam((x - 0.13, y - 0.13, 0), (x + 0.13, y + 0.13, h + 2.4), band=DARK_WOOD)
            b.block((x - 0.25, y - 0.25, 0), (x + 0.25, y + 0.25, 0.3), "rock", name="Pad", scale=1.0)
    for z in (2.4, 4.8):
        b.beam((-s, cy - s - 0.06, z), (s, cy - s + 0.06, z + 0.14), band=LIGHT_WOOD)
        b.beam((-s, cy + s - 0.06, z), (s, cy + s + 0.06, z + 0.14), band=LIGHT_WOOD)
        b.beam((-s - 0.06, cy - s, z), (-s + 0.06, cy + s, z + 0.14), band=LIGHT_WOOD)
        b.beam((s - 0.06, cy - s, z), (s + 0.06, cy + s, z + 0.14), band=LIGHT_WOOD)
    b.block((-s - 0.4, cy - s - 0.4, h), (s + 0.4, cy + s + 0.4, h + 0.2), "wood", name="Platform")
    for z in (h + 0.55, h + 1.0):
        for sy in (-1, 1):
            b.beam((-s - 0.35, cy + sy * (s + 0.35) - 0.04, z), (s + 0.35, cy + sy * (s + 0.35) + 0.04, z + 0.08),
                   band=LIGHT_WOOD)
        for sx in (-1, 1):
            b.beam((sx * (s + 0.35) - 0.04, cy - s - 0.35, z), (sx * (s + 0.35) + 0.04, cy + s + 0.35, z + 0.08),
                   band=LIGHT_WOOD)
    for x in (-0.3, 0.3):
        b.beam((x - 0.04, cy - s - 0.25, 0), (x + 0.04, cy - s - 0.17, h + 1.0), band=LIGHT_WOOD)
    for i in range(int(h / 0.4)):
        z = 0.3 + i * 0.4
        b.beam((-0.3, cy - s - 0.24, z), (0.3, cy - s - 0.18, z + 0.05), band=LIGHT_WOOD)
    b.solid("Roof", cone_mesh("Roof", (0, cy), (s + 0.6) * math.sqrt(2), h + 2.4, h + 4.0, 4, b.mats["tiles"],
                              rings=2, rot=math.pi / 4, flare=0.12, tile=(1.4, 1.2)))
    b.solid("Flagpole", cone_mesh("Flagpole", (0, cy), 0.04, h + 3.8, h + 5.2, 5, b.mats["iron"], rings=1))
    b.solid("Flag", poly_mesh("Flag", [[(0.04, cy, h + 5.1), (0.9, cy, h + 4.85), (0.04, cy, h + 4.6)]],
                              b.mats["cloth"]))
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = sx * s, cy + sy * s
            b.collide("leg", (x - 0.25, y - 0.25, 0), (x + 0.25, y + 0.25, h))
    b.front = (0.0, -1.0)
    b.inside = (0.0, cy)
    return b


@building
def log_cabin():
    """A Walden Woods cabin of round logs, a shingled roof, a stone
    chimney, and a porch."""
    b = Building("log_cabin", {"plaster": "white", "roof": "brown", "timber": "mid"})
    w, d, r = 6.0, 5.0, 0.17
    courses = 8
    for i in range(courses):
        z = r + i * 2 * r * 0.92
        for y in (0.0, d):
            b.solid("Log", log_mesh("Log", (-w / 2 - 0.35, y, z), (w / 2 + 0.35, y, z), r, b.mats["wood"]))
        zz = z + r * 0.92
        for x in (-w / 2, w / 2):
            b.solid("Log", log_mesh("Log", (x, -0.35, zz), (x, d + 0.35, zz), r, b.mats["wood"]))
    top = r + courses * 2 * r * 0.92
    ridge = slab_roof(b, w, d, top, 2.2, center=(0, d / 2), mat="tiles", over=0.6, thick=0.16, gable="boards")
    # Door and windows set in the front logs.
    b.block((-1.6, -0.3, 0.0), (-0.6, -0.12, 2.1), "boards", name="Door", scale=1.0)
    b.block((-1.72, -0.32, 0.0), (-1.6, -0.1, 2.2), "wood", name="Jamb", scale=1.0)
    b.block((-0.6, -0.32, 0.0), (-0.48, -0.1, 2.2), "wood", name="Jamb", scale=1.0)
    b.block((-1.72, -0.32, 2.1), (-0.48, -0.1, 2.25), "wood", name="Lintel", scale=1.0)
    # No window on the west wall: the chimney stands over it.
    for x, y, rot in ((1.4, -0.2, 0), (w / 2 + 0.2, 2.5, 90), (0.0, d + 0.2, 180)):
        m = frame(rot, (x, y, 1.5))
        b.solid("Window", box_mesh("Window", (-0.5, -0.08, -0.4), (0.5, 0.02, 0.4), b.mats["glass"], xf=m))
        for lo, hi in (((-0.6, -0.12, -0.5), (0.6, 0.0, -0.4)), ((-0.6, -0.12, 0.4), (0.6, 0.0, 0.5)),
                       ((-0.6, -0.12, -0.4), (-0.5, 0.0, 0.4)), ((0.5, -0.12, -0.4), (0.6, 0.0, 0.4)),
                       ((-0.03, -0.11, -0.4), (0.03, -0.01, 0.4))):
            b.solid("WindowFrame", box_mesh("WindowFrame", lo, hi, b.mats["wood"], band=LIGHT_WOOD, xf=m))
    # The stone chimney on the west gable.
    cx = -w / 2 - 0.55
    b.block((cx - 0.5, 1.9, 0), (cx + 0.5, 3.1, 2.4), "stone", name="Chimney", scale=2.0)
    b.block((cx - 0.35, 2.1, 2.4), (cx + 0.35, 2.9, ridge + 1.0), "stone", name="Chimney", scale=2.0)
    # The porch: a plank floor, two posts, and a lean-to.
    b.block((-w / 2, -1.8, 0), (w / 2, -0.2, 0.18), "wood", name="Porch")
    for x in (-w / 2 + 0.2, w / 2 - 0.2):
        b.beam((x - 0.1, -1.7, 0.18), (x + 0.1, -1.5, 2.3), band=DARK_WOOD)
    m = Matrix.Translation(Vector((0, -0.2, 2.6))) @ Matrix.Rotation(math.radians(-90), 4, "Z") @ Matrix.Rotation(
        math.atan2(0.45, 1.8), 4, "Y")
    b.solid("PorchRoof", box_mesh("PorchRoof", (0.0, -w / 2 - 0.2, -0.1), (2.0, w / 2 + 0.2, 0.0), b.mats["tiles"],
                                  xf=m))
    b.collide("cabin", (-w / 2 - 0.2, -0.2, 0), (w / 2 + 0.2, d + 0.2, top))
    b.collide("chimney", (cx - 0.5, 1.9, 0), (cx + 0.5, 3.1, 2.4))
    for x in (-w / 2 + 0.2, w / 2 - 0.2):
        b.collide("post", (x - 0.12, -1.72, 0), (x + 0.12, -1.48, 2.3))
    b.roofs.append(((0.0, d / 2), False, (w / 2 + 0.6, d / 2 + 0.6), top, ridge))
    b.front = (-1.1, -1.0)
    return b


@building
def gazebo():
    """An open octagonal garden gazebo on a raised floor, with railings and
    a tiled roof."""
    b = Building("gazebo", {"plaster": "white", "roof": "teal", "timber": "light"})
    R, h = 2.6, 2.6
    a = R * math.cos(math.pi / 8)
    cy = a + 0.3
    rot0 = math.radians(-67.5)
    b.solid("Floor", frustum_mesh("Floor", (0, cy), R + 0.15, R + 0.1, 0.0, 0.22, 8, b.mats["wood"], rot=rot0,
                                  band=LIGHT_WOOD))
    pts = []
    for k in range(8):
        ang = rot0 + math.pi * k / 4
        p = (R * math.cos(ang), cy + R * math.sin(ang))
        pts.append(p)
        b.block((p[0] - 0.09, p[1] - 0.09, 0.22), (p[0] + 0.09, p[1] + 0.09, h), "white_paint", name="Post")
    for k in range(8):
        if k == 7:  # The open side, between the two front posts.
            continue
        (x0, y0), (x1, y1) = pts[k], pts[(k + 1) % 8]
        for z in (0.6, 1.0):
            length = math.hypot(x1 - x0, y1 - y0)
            m = Matrix.Translation(Vector((x0, y0, z))) @ Matrix.Rotation(math.atan2(y1 - y0, x1 - x0), 4, "Z")
            b.solid("Rail", box_mesh("Rail", (0, -0.04, 0), (length, 0.04, 0.07), b.mats["white_paint"], xf=m))
    b.solid("Roof", cone_mesh("Roof", (0, cy), R + 0.6, h, h + 2.0, 8, b.mats["tiles"], rings=2, rot=rot0,
                              flare=0.15, tile=(1.4, 1.2)))
    b.solid("Cupola", frustum_mesh("Cupola", (0, cy), 0.5, 0.5, h + 1.4, h + 2.0, 8, b.mats["white_paint"],
                                   rot=rot0))
    b.solid("CupolaRoof", cone_mesh("CupolaRoof", (0, cy), 0.75, h + 2.0, h + 2.7, 8, b.mats["tiles"], rings=1,
                                    rot=rot0))
    b.solid("Finial", cone_mesh("Finial", (0, cy), 0.05, h + 2.6, h + 3.1, 5, b.mats["iron"], rings=1))
    b.block((-1.0, -0.25, 0), (1.0, 0.35, 0.11), "wood", name="Step")
    for x, y in pts:
        b.collide("post", (x - 0.12, y - 0.12, 0), (x + 0.12, y + 0.12, h))
    b.front = (0.0, -0.8)
    b.inside = (0.0, cy)
    return b


def thatched(name, scheme, w, d, fronts, sides, back, rise=3.0):
    """A one-storey timber-framed house under a thick thatched roof."""
    b = Building(name, scheme)
    b.box_storey(w, d, 0, fronts, sides=sides, back=back)
    top = STOREY + WALL_TOP
    # The thatch sits on the wall plates, clear of the kit walls' top beams.
    ridge = slab_roof(b, d, w, top + 0.3, rise, center=(0, d / 2), along_x=True, mat="thatch", over=0.8,
                      thick=0.45, gable="plaster", ridge_mat="thatch_dark", courses=(0.36, 0.72, 0.97))
    b.solid("RidgeRoll", log_mesh("RidgeRoll", (-w / 2 - 0.9, d / 2, ridge - 0.05), (w / 2 + 0.9, d / 2, ridge - 0.05),
                                  0.32, b.mats["thatch_dark"], sides=8))
    b.chimney(w / 2 - 1.4, d / 2 + 0.6, piece="Prop_Chimney2", above=1.0)
    b.collide("house", (-w / 2 - 0.1, -0.1, 0), (w / 2 + 0.1, d + 0.1, top))
    b.roofs.append(((0.0, d / 2), True, (d / 2 + 0.75, w / 2 + 0.75), top, ridge))
    return b


@building
def farmhouse():
    """A long farmhouse under thick thatch, for the farm edge by the orchard."""
    b = thatched("farmhouse", {"plaster": "white", "roof": "red", "timber": "dark"}, 10, 6,
                 ["G", "S", "Q", "S", "G"], ("GSG", "GSG"), "GPSPG", rise=3.2)
    b.front = (0.0, -0.9)
    return b


@building
def cottage_thatch():
    """A small thatched cottage with a round-topped door and shuttered windows."""
    b = thatched("cottage_thatch", {"plaster": "terracotta", "roof": "red", "timber": "mid"}, 6, 5,
                 ["W", "D", "W"], ("PSP", "PSP"), "PFP", rise=2.6)
    b.front = (-1.0, -0.9)
    return b


# --------------------------------------------------------------------------
# Third round: other roof shapes


def hip_roof(b, w, d, z, rise, center=(0.0, 0.0), over=0.5, mat="tiles"):
    """A hipped roof over a `w` by `d` plan, its ridge along X: two
    trapezoidal slopes, two triangular hips, and a soffit underneath.
    Returns the ridge's height."""
    cx, cy = center
    hx, hy = w / 2 + over, d / 2 + over
    ridge = max(0.0, hx - hy)
    zr = z + rise
    p = lambda x, y, zz: (cx + x, cy + y, zz)  # noqa: E731
    polys = [
        [p(-hx, -hy, z), p(hx, -hy, z), p(ridge, 0, zr), p(-ridge, 0, zr)],
        [p(hx, hy, z), p(-hx, hy, z), p(-ridge, 0, zr), p(ridge, 0, zr)],
        [p(-hx, hy, z), p(-hx, -hy, z), p(-ridge, 0, zr)],
        [p(hx, -hy, z), p(hx, hy, z), p(ridge, 0, zr)],
    ]
    b.solid("HipRoof", poly_mesh("HipRoof", polys, b.mats[mat], tile=2.0))
    soffit = [[p(-hx, -hy, z - 0.12), p(-hx, hy, z - 0.12), p(hx, hy, z - 0.12), p(hx, -hy, z - 0.12)]]
    b.solid("Soffit", poly_mesh("Soffit", soffit, b.mats["wood"], tile=2.0))
    # Fascia boards round the eaves close the gap to the soffit.
    for lo, hi in (((-hx, -hy - 0.02), (hx, -hy + 0.04)), ((-hx, hy - 0.04), (hx, hy + 0.02)),
                   ((-hx - 0.02, -hy), (-hx + 0.04, hy)), ((hx - 0.04, -hy), (hx + 0.02, hy))):
        b.beam((cx + lo[0], cy + lo[1], z - 0.14), (cx + hi[0], cy + hi[1], z + 0.02), band=DARK_WOOD)
    return zr


@building
def hip_house():
    """A square-set two-storey house under a hipped roof, with shuttered
    windows, a round-topped door, and a lantern by it: a shape no kit
    roof gives."""
    b = Building("hip_house", {"plaster": "sky", "roof": "slate", "timber": "dark"})
    w, d = 10, 8
    b.box_storey(w, d, 0, ["S", "W", "D", "W", "S"], sides=("PSPP", "PPSP"), back="PFPFP")
    b.box_storey(w, d, STOREY, ["t", "W", "W", "W", "t"], sides=("PWPP", "PPWP"), back="PFPFP")
    top = 2 * STOREY + WALL_TOP
    ridge = hip_roof(b, w, d, top, 2.4, center=(0, d / 2))
    b.chimney(-2.6, d / 2 + 1.2)
    lantern(b, 1.0, -0.05, 2.4)
    border(b, -5, 5, -0.05)
    b.collide("house", (-w / 2 - 0.1, -0.1, 0), (w / 2 + 0.1, d + 0.1, top))
    # The landing surface follows the long slopes; the hips fall away
    # inside its ends.
    b.roofs.append(((0.0, d / 2), True, (d / 2 + 0.5, 2.0), top, ridge))
    b.front = (-0.5, -0.9)
    return b


def gambrel_slab(b, a, c, length, cy, mat, thick=0.16, before=0.0, after=0.0):
    """One plane of a gambrel roof from (x, z) `a` to `c`, with `a` west of
    `c`, `length` long along Y, extended `before` and `after` along it."""
    dx, dz = c[0] - a[0], c[1] - a[1]
    run = math.hypot(dx, dz)
    xf = (Matrix.Translation(Vector((a[0], cy, a[1]))) @ Matrix.Rotation(math.atan2(-dz, dx), 4, "Y"))
    b.solid("Gambrel", box_mesh("Gambrel", (-before, -length / 2, -thick), (run + after, length / 2, 0.0),
                                b.mats[mat], tile=2.0, xf=xf))


@building
def gambrel_barn():
    """A red board barn for the farm under a gambrel roof, steep below its
    knees and shallow above, with big doors and a hayloft door in its
    front gable and a cupola on the ridge."""
    b = Building("gambrel_barn", {"plaster": "white", "roof": "charcoal", "timber": "dark"})
    w, d, wall = 10.0, 12.0, 3.6
    hx = w / 2
    b.block((-hx - 0.2, -0.2, 0), (hx + 0.2, d + 0.2, 0.3), "stone", name="Footing", scale=2.6)
    # Board walls, the front open for its doors.
    b.block((-hx, -0.1, 0.3), (-1.7, 0.1, wall), "barn", name="Wall")
    b.block((1.7, -0.1, 0.3), (hx, 0.1, wall), "barn", name="Wall")
    b.block((-1.7, -0.1, 3.1), (1.7, 0.1, wall), "barn", name="Wall")
    b.block((-hx, d - 0.1, 0.3), (hx, d + 0.1, wall), "barn", name="Wall")
    for sx in (-1, 1):
        b.block((sx * hx - 0.1, 0.1, 0.3), (sx * hx + 0.1, d - 0.1, wall), "barn", name="Wall")
    # The doors: two board leaves with white frames and cross braces.
    for sx in (-1, 1):
        x0, x1 = sorted((0.0, sx * 1.6))
        # The leaf starts behind its frame, so their edges don't share
        # planes.
        b.block((x0, -0.14, 0.3), (x1, -0.06, 3.05), "barn", name="Door")
        for lo, hi in (((x0, -0.2, 0.3), (x1, -0.14, 0.42)), ((x0, -0.2, 2.93), (x1, -0.14, 3.05)),
                       ((x0, -0.2, 0.3), (x0 + 0.12, -0.14, 3.05)), ((x1 - 0.12, -0.2, 0.3), (x1, -0.14, 3.05))):
            b.block(lo, hi, "white_paint", name="DoorFrame")
        mid = (x0 + x1) / 2
        m = Matrix.Translation(Vector((mid, -0.17, 1.675))) @ Matrix.Rotation(math.atan2(2.6, 1.5), 4, "Y")
        b.solid("Brace", box_mesh("Brace", (-1.5, -0.03, -0.06), (1.5, 0.03, 0.06), b.mats["white_paint"], xf=m))
    # The gambrel: eaves at the walls, knees 2.3 m higher and 1.7 m in, and
    # a shallow pitch to the ridge.
    eave, knee, ridge_z = (hx, wall), (hx - 1.7, wall + 2.3), wall + 3.5
    for side in (-1, 1):
        lower = ((side * eave[0], eave[1]), (side * knee[0], knee[1]))
        upper = ((side * knee[0], knee[1]), (0.0, ridge_z))
        for (a, c), over in ((lower, 0.45), (upper, 0.0)):
            if side > 0:
                a, c = c, a
            before, after = (over, 0.0) if side < 0 else (0.0, over)
            gambrel_slab(b, a, c, d + 0.7, d / 2, "tiles", before=before, after=after)
    for y, flip in ((-0.1, False), (d + 0.1, True)):
        pent = [(-hx, y, wall), (hx, y, wall), (knee[0], y, knee[1] - 0.1), (0.0, y, ridge_z - 0.12),
                (-knee[0], y, knee[1] - 0.1)]
        b.solid("Gable", poly_mesh("Gable", [pent[::-1] if flip else pent], b.mats["barn"]))
    # The hayloft door and its frame.
    b.block((-0.7, -0.1, 4.2), (0.7, -0.02, 5.6), "barn", name="Loft")
    for lo, hi in (((-0.8, -0.14, 4.1), (0.8, -0.06, 4.2)), ((-0.8, -0.14, 5.6), (0.8, -0.06, 5.7)),
                   ((-0.8, -0.14, 4.1), (-0.7, -0.06, 5.7)), ((0.7, -0.14, 4.1), (0.8, -0.06, 5.7))):
        b.block(lo, hi, "white_paint", name="LoftFrame")
    b.beam((-0.08, -0.9, 5.75), (0.08, 0.0, 5.9), band=DARK_WOOD, name="HayBeam")
    # A louvred cupola on the ridge.
    b.block((-0.6, d / 2 - 0.6, ridge_z - 0.2), (0.6, d / 2 + 0.6, ridge_z + 0.9), "white_paint", name="Cupola")
    b.solid("CupolaRoof", cone_mesh("CupolaRoof", (0, d / 2), 1.0, ridge_z + 0.9, ridge_z + 1.7, 4,
                                    b.mats["tiles"], rings=1, rot=math.pi / 4))
    b.collide("barn", (-hx - 0.2, -0.2, 0), (hx + 0.2, d + 0.2, wall))
    # Under the steep lower slopes, so a lander stops near their surface.
    b.collide("loft", (-hx + 0.6, -0.2, 0), (hx - 0.6, d + 0.2, 4.9))
    # The landing surface is the shallow upper roof, from knee to knee;
    # the steep lower slopes shed a lander onto the walls' tops.
    b.roofs.append(((0.0, d / 2), False, (knee[0], d / 2 + 0.35), knee[1], ridge_z))
    b.front = (2.6, -1.0)
    return b


def main():
    global KIT
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if "--kit" in args:
        i = args.index("--kit")
        KIT = os.path.expanduser(args[i + 1])
        del args[i:i + 2]
    out = args[0] if args else "assets/verse/generated/buildings"
    names = args[1:] or list(BUILDINGS)
    for name in names:
        b = BUILDINGS[name]()
        b.save(out)
    fail_on_flickers()


if __name__ == "__main__":
    main()
