"""Shared helpers for Verse's particle sprite sheets.

Each sprite family script under `scripts/blender/fx/` builds a small scene,
renders one cell per frame with Cycles, and packs the cells into one square
sheet with `write_sheet`. Run them all with `scripts/blender/build-fx.sh`.

Encoding (`docs/verse/particles.md`): a sheet is an 8-bit RGBA PNG whose RGB
is the cell's *premultiplied* linear color, sRGB-encoded, and whose alpha is
linear coverage. Verse uploads it as an sRGB texture, so the GPU decodes the
premultiplied linear color and mipmaps filter it correctly. Additive
particles add RGB and ignore alpha; alpha-blended ones composite RGB over
the scene with alpha. Fire keeps its glow where coverage is low, which a
straight-alpha PNG cannot store.
"""

import math
import os
import struct
import sys
import tempfile
import zlib

import bpy
import numpy as np


def args():
    """The arguments after `--`."""
    return sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


def reset(cell, samples=48, extent=1.0):
    """An empty Cycles scene rendering `cell`-pixel squares with a
    transparent film, seen by an orthographic camera on -Z covering
    `extent` units each way from the origin."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    s = bpy.context.scene
    s.render.engine = "CYCLES"
    s.cycles.device = "CPU"
    s.cycles.samples = samples
    s.cycles.use_denoising = samples >= 32
    s.cycles.denoiser = "OPENIMAGEDENOISE"
    s.cycles.seed = 7
    s.cycles.volume_step_rate = 0.5
    s.cycles.volume_max_steps = 512
    s.cycles.max_bounces = 4
    s.cycles.volume_bounces = 1
    s.render.film_transparent = True
    s.render.resolution_x = cell
    s.render.resolution_y = cell
    s.render.resolution_percentage = 100
    s.render.image_settings.file_format = "OPEN_EXR"
    s.render.image_settings.color_depth = "32"
    s.view_settings.view_transform = "Standard"
    s.world = bpy.data.worlds.new("world")
    s.world.use_nodes = True
    s.world.node_tree.nodes["Background"].inputs[1].default_value = 0.0
    cam = bpy.data.objects.new("camera", bpy.data.cameras.new("camera"))
    s.collection.objects.link(cam)
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = 2.0 * extent
    cam.location = (0.0, 0.0, 10.0)
    cam.data.clip_end = 40.0
    s.camera = cam
    return s


def ambient(strength, color=(1.0, 1.0, 1.0)):
    """A uniform world light that shades smoke's undersides softly. It
    lights the scene but the transparent film hides it."""
    bg = bpy.context.scene.world.node_tree.nodes["Background"]
    bg.inputs[0].default_value = (*color, 1.0)
    bg.inputs[1].default_value = strength


def sun(direction, strength, color=(1.0, 1.0, 1.0)):
    """A sun shining along `direction` (from above the frame, say)."""
    light = bpy.data.lights.new("sun", "SUN")
    light.energy = strength
    light.color = color
    light.angle = math.radians(8.0)
    obj = bpy.data.objects.new("sun", light)
    bpy.context.scene.collection.objects.link(obj)
    from mathutils import Vector

    obj.rotation_euler = (-Vector(direction)).to_track_quat("Z", "Y").to_euler()
    return obj


class Nodes:
    """A tiny builder over a material's node tree."""

    def __init__(self, material):
        material.use_nodes = True
        self.tree = material.node_tree
        for node in list(self.tree.nodes):
            self.tree.nodes.remove(node)

    def node(self, kind, **inputs):
        n = self.tree.nodes.new(kind)
        for key, value in inputs.items():
            if key.startswith("_"):
                setattr(n, key[1:], value)
            else:
                self.set(n.inputs[key], value)
        return n

    def set(self, socket, value):
        if hasattr(value, "bl_idname") or isinstance(value, bpy.types.NodeSocket):
            out = value if isinstance(value, bpy.types.NodeSocket) else value.outputs[0]
            self.tree.links.new(out, socket)
        else:
            socket.default_value = value

    def math(self, op, a, b=0.0, clamp=False):
        n = self.tree.nodes.new("ShaderNodeMath")
        n.operation = op
        n.use_clamp = clamp
        self.set(n.inputs[0], a)
        self.set(n.inputs[1], b)
        return n.outputs[0]

    def link(self, out, socket):
        self.tree.links.new(out, socket)


def material(name):
    m = bpy.data.materials.new(name)
    return m, Nodes(m)


def domain(radius, material):
    """A sphere a volume material fills."""
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=4, radius=radius)
    obj = bpy.context.object
    obj.data.materials.append(material)
    return obj


def plane(size, material):
    bpy.ops.mesh.primitive_plane_add(size=size)
    obj = bpy.context.object
    obj.data.materials.append(material)
    return obj


def render():
    """Renders the scene and returns its premultiplied linear RGBA, top row
    first, as a float array of shape (cell, cell, 4)."""
    s = bpy.context.scene
    fd, path = tempfile.mkstemp(suffix=".exr")
    os.close(fd)
    s.render.filepath = path
    bpy.ops.render.render(write_still=True)
    img = bpy.data.images.load(path)
    w, h = img.size
    pixels = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)[::-1]
    bpy.data.images.remove(img)
    os.remove(path)
    return pixels


def srgb(linear):
    linear = np.clip(linear, 0.0, 1.0)
    return np.where(
        linear <= 0.0031308, linear * 12.92, 1.055 * np.power(linear, 1.0 / 2.4) - 0.055
    )


def write_png(path, rgba8):
    """Writes an RGBA8 array (rows top first) as a PNG, without Blender's
    color management in the way."""
    h, w, _ = rgba8.shape
    raw = b"".join(b"\x00" + rgba8[y].tobytes() for y in range(h))

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


def write_sheet(path, cells, columns, gain=None):
    """Packs `cells` (premultiplied linear RGBA arrays of one size) row by
    row into a sheet `columns` wide and writes it. Unless `gain` is given,
    the color is scaled so the brightest cell's 99.9th percentile channel
    reaches 1. Returns the gain used."""
    cell = cells[0].shape[0]
    rows = math.ceil(len(cells) / columns)
    sheet = np.zeros((rows * cell, columns * cell, 4), dtype=np.float32)
    for i, c in enumerate(cells):
        y, x = divmod(i, columns)
        sheet[y * cell : (y + 1) * cell, x * cell : (x + 1) * cell] = c
    if gain is None:
        peak = float(np.percentile(sheet[..., :3], 99.9))
        gain = 1.0 / max(peak, 1e-6)
    rgb = sheet[..., :3] * gain
    alpha = np.clip(sheet[..., 3:], 0.0, 1.0)
    # Premultiplied color can't exceed coverage for alpha-blended texels,
    # but a glow may: it is stored as it is and drawn additively.
    out = np.concatenate([srgb(rgb), alpha], axis=2)
    write_png(path, np.round(out * 255.0).astype(np.uint8))
    preview = os.environ.get("FX_PREVIEW")
    if preview:
        # The sheet over a dusk-grey field, alpha-blended on the left half
        # of each pair and added on the right, for a quick look.
        bg = np.array([0.05, 0.06, 0.08], dtype=np.float32)
        over = rgb + bg * (1.0 - alpha)
        added = rgb + bg
        both = np.concatenate([over, added], axis=1)
        os.makedirs(preview, exist_ok=True)
        name = os.path.splitext(os.path.basename(path))[0]
        img = np.concatenate([srgb(both), np.ones_like(both[..., :1])], axis=2)
        write_png(os.path.join(preview, name + ".png"), np.round(img * 255.0).astype(np.uint8))
    print(f"SHEET {path} {columns}x{rows} cells of {cell} px, gain {gain:.4f}")
    return gain


def ease_out(x):
    return 1.0 - (1.0 - x) ** 2
