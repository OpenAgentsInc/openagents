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
}
ROOF = {
    "red": None,
    "brown": (0.52, 0.33, 0.22),
    "slate": (0.33, 0.37, 0.44),
    "green": (0.42, 0.50, 0.42),
}
TIMBER = {"light": 1.0, "mid": 0.75, "dark": 0.52}

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


class Building:
    """One building under construction, plus its collision boxes."""

    def __init__(self, name, scheme):
        bpy.ops.wm.read_factory_settings(use_empty=True)
        self.name = name
        self.mats = make_materials(scheme)
        self.kit = Kit(self.mats)
        self.col = bpy.context.scene.collection
        self.boxes = []
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
        side_bay = depth / n_side
        sides = sides or "P" * n_side
        left, right = (sides, sides) if isinstance(sides, str) else sides
        # Pad or trim each side to its bay count.
        left, right = [list(s) + ["P"] * n_side for s in (left, right)]
        back = back or "P" * int(w / 2)
        if "front" not in skip:
            self.run(fronts, (x0, y0, z), 0, z, height=height, stone=stone)
        if "back" not in skip:
            self.run(back, (x0 + w, d, z), 180, z, height=height, stone=stone)
        if "left" not in skip:
            self.run(left[:n_side], (x0, d, z), -90, z, bay=side_bay, height=height, stone=stone)
        if "right" not in skip:
            self.run(right[:n_side], (x0 + w, y0, z), 90, z, bay=side_bay, height=height, stone=stone)
        pts = [(x0, y0), (x0 + w, y0), (x0, d), (x0 + w, d)]
        self.corners(pts, z, height, corner)

    def jetty(self, w, z, out, x0=None, y=0.0):
        """The underside of a jettied floor: joist ends, soffit, and beams."""
        x0 = -w / 2 if x0 is None else x0
        n = max(2, int(round(w / 1.7)))
        for i in range(n + 1):
            x = x0 + 0.1 + (w - 0.2) * i / n
            self.put("Roof_Support2", (x, -0.09 + y, z + 0.02), 0, (1, out / 0.69, 1))
        # Soffit boards between the wall and the jettied floor.
        for i in range(int(w / 2)):
            self.put("Floor_WoodDark", (x0 + 1 + 2 * i, -out / 2 + y, z - 0.03), 0,
                     (1, (out + 0.1) / 2, 1))
        self.beam((x0 - 0.05, -out - 0.06 + y, z - 0.14), (x0 + w + 0.05, -out + 0.16 + y, z + 0.02))

    # -- generated solids -------------------------------------------------

    def beam(self, lo, hi, band=LIGHT_WOOD, mat="wood", name="Beam"):
        mesh = box_mesh(name, lo, hi, self.mats[mat], band=band)
        return self.solid(name, mesh)

    def block(self, lo, hi, mat, name="Block", scale=2.0):
        mesh = box_mesh(name, lo, hi, self.mats[mat], tile=scale)
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


def box_mesh(name, lo, hi, mat, band=None, tile=2.0, xf=None):
    """A box with UVs: along a wood band, or tiled at `tile` metres.

    `xf` moves the finished box, for boxes built in a wall's local frame.
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
    for f, axis in zip(faces, normals):
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
    b.block(lo, hi, "plaster", name="TurretBody")
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
    b.run([("G", 2), ("G", 2)], (bx + 1.2, 2.0, base), 90, base)
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
    b.beam((-w / 2, 0.0, STOREY - 0.06), (w / 2, d, STOREY - 0.04), band=DARK_WOOD)
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
        b.run(front, (0, 4, z), 0, z)
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


if __name__ == "__main__":
    main()
